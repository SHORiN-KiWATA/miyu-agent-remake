//! 密钥（施工 8-5，`docs/blueprint/config.md`「协议」`secret.*`、「怎么走」第九条）：写、换、删、列（`used_by`、配置里引用了
//! 却没设的也列）；参数不对的几种；Unix 上 0600；值不进回应、报错、系统日志；引用的密钥、环境变量取不到的报警告，
//! `{ env }` 照核心起来时的环境；密钥文件写错的算进 `config_errors`、写不了；手改被看到；写之前的手改先记。

mod support;

use serde_json::{Value, json};

use miyu_config::secret::Reference;
use miyu_config::{Applies, Control, Item, Kind, Layer, Ui, Value as ConfigValue};
use miyu_session::testkit::Script;

use support::*;

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";
const FAKE_2: &str = "sk-FAKE-KEY-FOR-TESTS-0002";

/// 密钥文件、系统日志在数据根里的位置。
const SECRETS: &str = "system/secrets.toml";
const JOURNAL: &str = "system/journal.jsonl";

/// 中文的开头注释。
const HEADER: &str = "# Miyu 的密钥：只经 Miyu 写入、替换、删除。不要把这份文件贴给别人。\n";

/// 类型是密钥的一项：出厂的清单里还没有，测试自己加。
fn secret_item(key: &'static str) -> Item {
    Item {
        key,
        kind: Kind::Secret,
        default: Some(ConfigValue::Secret(Reference::Env(
            "EXAMPLE_KEY".to_string(),
        ))),
        layers: &[Layer::System, Layer::Personal],
        tighten: None,
        env: None,
        applies: Applies::NewSession,
        ui: Ui {
            page: "general",
            group: "display",
            common: false,
            control: Control::Select,
            hidden: false,
        },
    }
}

/// 连上一个清单里有两项密钥的核心，环境是 `env`，握过手。
async fn client(home: &Home, env: &[(&str, &str)]) -> Client {
    let extra = [
        secret_item("providers.demo"),
        secret_item("providers.other"),
    ];
    let core = home.core_with_items(&Script::new([]), None, env, &extra);
    let mut client = Client::connect(core);
    client.hello().await;
    client
}

fn read(home: &Home, relative: &str) -> Option<String> {
    std::fs::read_to_string(home.root.path().join(relative)).ok()
}

fn journal(home: &Home) -> Vec<Value> {
    read(home, JOURNAL)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("一行一条 JSON"))
        .collect()
}

/// 看过的字里都没有假 key。
fn clean(what: &str, seen: &str) {
    for key in [FAKE, FAKE_2, "sk-FAKE"] {
        assert!(!seen.contains(key), "{what} 里有密钥：{seen}");
    }
}

async fn set(client: &mut Client, id: &str, name: &str, value: &str) -> Value {
    let reply = client
        .call(id, "secret.set", json!({"name": name, "value": value}))
        .await;
    clean("回应", &reply.to_string());
    reply
}

#[tokio::test]
async fn setting_replacing_and_deleting_answer_and_leave_a_trail() {
    let home = Home::new();
    let mut client = client(&home, &[]).await;
    let first = set(&mut client, "secret-1", "deepseek", &format!("  {FAKE}\n")).await;
    assert_eq!(first["result"], json!({"replaced": false}), "{first}");
    assert_eq!(
        read(&home, SECRETS).as_deref(),
        Some(format!("{HEADER}deepseek = \"{FAKE}\"\n").as_str()),
        "新建的带开头的注释，存的是去掉前后空白的"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(home.root.path().join(SECRETS))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let second = set(&mut client, "secret-2", "deepseek", FAKE_2).await;
    assert_eq!(second["result"], json!({"replaced": true}));
    let other = set(&mut client, "secret-3", "bigmodel-2", FAKE).await;
    assert_eq!(other["result"], json!({"replaced": false}));
    assert_eq!(
        read(&home, SECRETS).as_deref(),
        Some(format!("{HEADER}deepseek = \"{FAKE_2}\"\nbigmodel-2 = \"{FAKE}\"\n").as_str())
    );
    let deleted = client
        .call("secret-4", "secret.delete", json!({"name": "deepseek"}))
        .await;
    assert_eq!(deleted["result"], json!({}), "{deleted}");
    let again = client
        .call("secret-5", "secret.delete", json!({"name": "deepseek"}))
        .await;
    assert_eq!(reason(&again), Some("unknown_secret"));
    assert_eq!(again["error"]["message"], "没有这个密钥。");
    assert_eq!(
        read(&home, SECRETS).as_deref(),
        Some(format!("{HEADER}bigmodel-2 = \"{FAKE}\"\n").as_str())
    );
    let lines = journal(&home);
    let seen: Vec<(Value, Value, Value, Value)> = lines
        .iter()
        .map(|line| {
            (
                line["kind"].clone(),
                line["body"].clone(),
                line["by"].clone(),
                line["cause"].clone(),
            )
        })
        .collect();
    let alice = json!({"kind": "person", "account": "alice"});
    let body = |name: &str, action: &str| json!({"name": name, "action": action, "via": "set"});
    assert_eq!(
        seen,
        [
            (
                json!("secret.changed"),
                body("deepseek", "set"),
                alice.clone(),
                json!("secret-1")
            ),
            (
                json!("secret.changed"),
                body("deepseek", "replaced"),
                alice.clone(),
                json!("secret-2")
            ),
            (
                json!("secret.changed"),
                body("bigmodel-2", "set"),
                alice.clone(),
                json!("secret-3")
            ),
            (
                json!("secret.changed"),
                body("deepseek", "deleted"),
                alice,
                json!("secret-4")
            ),
        ]
    );
    clean("系统日志", &read(&home, JOURNAL).unwrap());
}

#[tokio::test]
async fn bad_params_are_refused_and_nothing_is_written() {
    let home = Home::new();
    let mut client = client(&home, &[]).await;
    let too_big = format!("sk-FAKE-{}", "x".repeat(16 * 1024));
    let cases = [
        json!({"name": "DeepSeek", "value": FAKE}),
        json!({"name": "", "value": FAKE}),
        json!({"name": "a".repeat(65), "value": FAKE}),
        json!({"name": "deepseek", "value": " \n "}),
        json!({"name": "deepseek", "value": "sk-FAKE\u{1b}[2J"}),
        json!({"name": "deepseek", "value": too_big}),
        json!({"name": "deepseek"}),
        json!({"name": "deepseek", "value": 3}),
        json!({"value": FAKE}),
    ];
    for (n, params) in cases.into_iter().enumerate() {
        let reply = client
            .call(&format!("bad-{n}"), "secret.set", params.clone())
            .await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}");
        clean("拒绝", &reply.to_string());
    }
    let reply = client.call("bad-delete", "secret.delete", json!({})).await;
    assert_eq!(reason(&reply), Some("bad_params"));
    assert_eq!(read(&home, SECRETS), None, "什么都没写");
    assert_eq!(read(&home, JOURNAL), None);
}

#[tokio::test]
async fn the_list_has_names_whether_set_and_who_uses_them() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[providers]\ndemo = { secret = \"deepseek\" }\nother = { secret = \"bigmodel-2\" }\n",
    );
    home.write(
        "home/alice/settings.toml",
        "[providers]\nother = { secret = \"deepseek\" }\n",
    );
    let mut client = client(&home, &[]).await;
    set(&mut client, "secret-1", "zeta", FAKE).await;
    let listed = client.call("list-1", "secret.list", json!({})).await;
    assert_eq!(
        listed["result"],
        json!({"secrets": [
            {"name": "deepseek", "set": false, "used_by": ["providers.demo", "providers.other"]},
            {"name": "zeta", "set": true, "used_by": []},
        ]}),
        "照最终值找引用：个人设置盖掉的不算"
    );
    set(&mut client, "secret-2", "deepseek", FAKE_2).await;
    let listed = client.call("list-2", "secret.list", json!({})).await;
    assert_eq!(listed["result"]["secrets"][0]["set"], true);
    clean("列出", &listed.to_string());
}

#[tokio::test]
async fn missing_references_warn_until_set_and_env_is_the_cores() {
    let home = Home::new();
    home.write(
        "system/config.toml",
        "[providers]\ndemo = { secret = \"deepseek\" }\nother = { env = \"MISSING_KEY\" }\n",
    );
    let mut client = client(&home, &[("PRESENT_KEY", FAKE)]).await;
    let problems = |reply: &Value| -> Vec<(Value, Value, Value)> {
        reply["result"]["problems"]
            .as_array()
            .unwrap()
            .iter()
            .map(|problem| {
                (
                    problem["code"].clone(),
                    problem["key"].clone(),
                    problem["level"].clone(),
                )
            })
            .collect()
    };
    let got = client.call("get-1", "config.get", json!({})).await;
    assert_eq!(
        problems(&got),
        [
            (
                json!("unknown_secret"),
                json!("providers.demo"),
                json!("warning")
            ),
            (
                json!("env_not_set"),
                json!("providers.other"),
                json!("warning")
            ),
        ]
    );
    assert_eq!(
        got["result"]["problems"][0]["message"],
        "providers.demo 引用的密钥 deepseek 还没设。"
    );
    assert_eq!(got["result"]["problems"][0]["file"], "system/config.toml");
    assert_eq!(
        got["result"]["files"]["secrets"],
        json!({"file": SECRETS}),
        "只说在哪，不给版本"
    );
    set(&mut client, "secret-1", "deepseek", FAKE).await;
    let checked = client
        .call(
            "check-1",
            "config.check",
            json!({"layer": "personal", "text": "[providers]\ndemo = { secret = \"deepseek\" }\nother = { env = \"PRESENT_KEY\" }\n"}),
        )
        .await;
    assert_eq!(
        checked["result"]["problems"],
        json!([]),
        "设了的、核心的环境里有的不报"
    );
    let got = client.call("get-2", "config.get", json!({})).await;
    assert_eq!(
        problems(&got),
        [(
            json!("env_not_set"),
            json!("providers.other"),
            json!("warning")
        )]
    );
    clean("config.get", &got.to_string());
}

#[tokio::test]
async fn a_broken_secrets_file_counts_as_errors_and_blocks_writes() {
    let home = Home::new();
    home.write(
        SECRETS,
        &format!("DeepSeek = \"{FAKE}\"\nok = \"{FAKE_2}\"\n"),
    );
    let core = home.core_configured(&Script::new([]), None, &[]);
    let mut client = Client::connect(core);
    let hello = client.hello().await;
    assert_eq!(hello["result"]["config_errors"], 1, "{hello}");
    let got = client.call("get-1", "config.get", json!({})).await;
    let problem = &got["result"]["problems"][0];
    assert_eq!(problem["code"], "bad_format");
    assert_eq!(problem["file"], SECRETS);
    assert_eq!(problem["key"], "DeepSeek");
    assert_eq!(problem["line"], 1);
    assert!(problem.get("got").is_none(), "{problem}");
    clean("config.get", &got.to_string());
    let listed = client.call("list-1", "secret.list", json!({})).await;
    assert_eq!(
        listed["result"],
        json!({"secrets": [{"name": "ok", "set": true, "used_by": []}]})
    );

    home.write(SECRETS, &format!("deepseek = \"{FAKE}\n"));
    let reply = set(&mut client, "secret-1", "other", FAKE_2).await;
    assert_eq!(reason(&reply), Some("config_file_broken"), "{reply}");
    assert_eq!(reply["error"]["data"]["problems"][0]["code"], "syntax");
    assert_eq!(
        reply["error"]["data"]["problems"][0]["using"],
        json!({"from": "last_good"})
    );
    assert_eq!(
        read(&home, SECRETS).as_deref(),
        Some(format!("deepseek = \"{FAKE}\n").as_str()),
        "一个字节不动"
    );
    let listed = client.call("list-2", "secret.list", json!({})).await;
    assert_eq!(
        listed["result"]["secrets"][0]["name"], "ok",
        "照上一次读好的用"
    );
    clean("日志", &read(&home, JOURNAL).unwrap_or_default());
}

#[tokio::test]
async fn a_hand_edit_before_a_set_is_recorded_first_and_kept() {
    let home = Home::new();
    let mut client = client(&home, &[]).await;
    home.write(SECRETS, &format!("# 手写的\nhand = \"{FAKE}\"\n"));
    let reply = set(&mut client, "secret-1", "deepseek", FAKE_2).await;
    assert_eq!(reply["result"], json!({"replaced": false}));
    assert_eq!(
        read(&home, SECRETS).as_deref(),
        Some(format!("# 手写的\nhand = \"{FAKE}\"\ndeepseek = \"{FAKE_2}\"\n").as_str()),
        "在手改的字上改，不另加开头的注释"
    );
    let lines = journal(&home);
    assert_eq!(
        lines[0]["body"],
        json!({"name": "hand", "action": "set", "via": "file"})
    );
    assert_eq!(lines[0]["by"], json!({"kind": "kernel"}));
    assert!(lines[0].get("cause").is_none());
    assert_eq!(
        lines[1]["body"],
        json!({"name": "deepseek", "action": "set", "via": "set"})
    );
}

#[tokio::test]
async fn hand_edits_are_watched() {
    let home = Home::new();
    home.root.prepare_home(&alice()).expect("建得了家目录");
    let core = home.core_configured(&Script::new([]), None, &[]);
    let _watching = core.watch_config().expect("监视得了");
    let mut client = Client::connect(core);
    client.hello().await;
    let steps = [
        (
            format!("a = \"{FAKE}\"\n"),
            json!({"name": "a", "action": "set", "via": "file"}),
        ),
        (
            format!("a = \"{FAKE_2}\"\n"),
            json!({"name": "a", "action": "replaced", "via": "file"}),
        ),
        (
            "# 空了\n".to_string(),
            json!({"name": "a", "action": "deleted", "via": "file"}),
        ),
    ];
    for (n, (text, body)) in steps.into_iter().enumerate() {
        home.write(SECRETS, &text);
        until("手改被看到", || journal(&home).len() > n).await;
        assert_eq!(journal(&home)[n]["body"], body);
    }
    let listed = client.call("list-1", "secret.list", json!({})).await;
    assert_eq!(listed["result"], json!({"secrets": []}));
    clean("系统日志", &read(&home, JOURNAL).unwrap());
}

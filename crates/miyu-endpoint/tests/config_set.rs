//! 改配置（施工 8-3，`docs/blueprint/config.md`「协议」的 `config.set`、「怎么走」第五、六条）：只动那一项、新文件第一行是
//! `#:schema`；回应的样子；每一种拒绝；`expect`、整份换的版本；写之前发现手改过先重读；一次几项全收或者全不收；写不成
//! 什么都没变；日志记一条。

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_store::config_file::version;

use crate::support::*;

/// 个人设置在数据根里的位置。
const PERSONAL: &str = "home/alice/settings.toml";

/// 连上一个照磁盘上的配置起来的核心，握过手。
async fn client(home: &Home) -> Client {
    let mut client = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    client.hello().await;
    client
}

/// 一份文件现在的字；没有的是空的。
fn read(home: &Home, relative: &str) -> Option<String> {
    std::fs::read_to_string(home.root.path().join(relative)).ok()
}

/// 一份日志里的每一条。
fn journal(home: &Home, relative: &str) -> Vec<Value> {
    read(home, relative)
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("一行一条 JSON"))
        .collect()
}

async fn set(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "config.set", params).await
}

#[tokio::test]
async fn a_new_file_is_made_with_the_schema_line_and_the_change_is_logged() {
    let home = Home::new();
    let mut client = client(&home).await;
    let reply = set(
        &mut client,
        "config-9f2c4e1a7b3d5f60-1",
        json!({"layer": "personal", "changes": [{"key": "ui.language", "input": "zh"}]}),
    )
    .await;
    let text = "#:schema ../../state/config/settings.schema.json\n\n[ui]\nlanguage = \"zh\"\n";
    assert_eq!(read(&home, PERSONAL).as_deref(), Some(text));
    assert_eq!(
        reply["result"],
        json!({
            "keys": {"ui.language": {
                "applies": "now",
                "effective": "zh",
                "origin": {"file": PERSONAL, "layer": "personal", "line": 4},
                "value": "zh",
            }},
            "version": version(text.as_bytes()),
        })
    );
    let logged = journal(&home, "home/alice/journal.jsonl");
    assert_eq!(logged.len(), 1);
    assert_eq!(logged[0]["seq"], 1);
    assert_eq!(logged[0]["kind"], "config.changed");
    assert_eq!(
        logged[0]["by"],
        json!({"kind": "person", "account": "alice"})
    );
    assert_eq!(logged[0]["cause"], "config-9f2c4e1a7b3d5f60-1");
    assert_eq!(
        logged[0]["body"],
        json!({"layer": "personal", "file": PERSONAL, "via": "set",
               "changes": [{"key": "ui.language", "new": "zh"}]})
    );
    let mut fresh = Client::connect(home.core_configured(&Script::new([]), None, &[]));
    assert_eq!(fresh.hello().await["result"]["language"], "zh");
    assert_eq!(
        client.hello().await["result"]["language"],
        "zh",
        "同一个核心照新的"
    );
}

#[tokio::test]
async fn only_that_item_changes_and_the_system_config_goes_to_the_system_log() {
    let home = Home::new();
    let system =
        "# 管理员写的\n[log]\nlevel = \"info\"   # 平时\n\n[ui]\nlanguage = \"en\"\nweird = 1\n";
    home.write("system/config.toml", system);
    let mut client = client(&home).await;
    let reply = set(
        &mut client,
        "c1",
        json!({"layer": "system", "changes": [
            {"key": "log.level", "value": "debug"},
            {"key": "ui.language", "unset": true},
        ]}),
    )
    .await;
    let after = "# 管理员写的\n[log]\nlevel = \"debug\"   # 平时\n\n[ui]\nweird = 1\n";
    assert_eq!(read(&home, "system/config.toml").as_deref(), Some(after));
    let keys = &reply["result"]["keys"];
    assert_eq!(keys["log.level"]["value"], "debug");
    assert_eq!(
        keys["ui.language"],
        json!({"applies": "now", "effective": "auto", "origin": {"layer": "default"}}),
        "删掉的没有 value，照下面一层"
    );
    let logged = journal(&home, "system/journal.jsonl");
    assert_eq!(
        logged[0]["body"]["changes"],
        json!([{"key": "log.level", "old": "info", "new": "debug"}, {"key": "ui.language", "old": "en"}]),
        "一次一条，照键名排"
    );
    assert!(read(&home, "home/alice/journal.jsonl").is_none());
}

#[tokio::test]
async fn nothing_changes_when_it_already_is() {
    let home = Home::new();
    home.write(PERSONAL, "[ui]\nlanguage = \"zh\"\n");
    let mut client = client(&home).await;
    let reply = set(
        &mut client,
        "c1",
        json!({"layer": "personal", "changes": [
            {"key": "ui.language", "input": "\"zh\""},
            {"key": "permission.start_read_only", "unset": true},
        ]}),
    )
    .await;
    assert_eq!(
        reply["result"],
        json!({"keys": {}, "version": version(b"[ui]\nlanguage = \"zh\"\n")})
    );
    assert!(
        read(&home, "home/alice/journal.jsonl").is_none(),
        "没写就不记"
    );
    let reply = set(
        &mut client,
        "c2",
        json!({"layer": "system", "changes": [{"key": "ui.language", "unset": true}]}),
    )
    .await;
    assert_eq!(reply["result"], json!({"keys": {}, "version": null}));
    assert!(
        read(&home, "system/config.toml").is_none(),
        "删一项不新建文件"
    );
}

#[tokio::test]
async fn malformed_requests_are_bad_params() {
    let home = Home::new();
    let mut client = client(&home).await;
    let change = json!({"key": "ui.language", "input": "zh"});
    for params in [
        json!({"layer": "personal"}),
        json!({"layer": "personal", "changes": []}),
        json!({"layer": "personal", "changes": [change], "text": ""}),
        json!({"layer": "project", "changes": [change]}),
        json!({"layer": "personal", "text": "[ui]\n"}),
        json!({"layer": "personal", "changes": [{"key": "ui.language"}]}),
        json!({"layer": "personal", "changes": [{"key": "ui.language", "input": "zh", "value": "zh"}]}),
        json!({"layer": "personal", "changes": [{"key": "ui.language", "unset": false}]}),
        json!({"layer": "personal", "changes": [change, change]}),
    ] {
        let reply = set(&mut client, "c1", params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}");
    }
    assert!(read(&home, PERSONAL).is_none());
}

#[tokio::test]
async fn one_bad_item_stops_them_all() {
    let home = Home::new();
    home.write(PERSONAL, "[ui]\nlanguage = \"zh\"\n");
    let mut client = client(&home).await;
    let unknown = set(
        &mut client,
        "c1",
        json!({"layer": "personal", "changes": [{"key": "ui.langauge", "input": "zh"}]}),
    )
    .await;
    assert_eq!(reason(&unknown), Some("unknown_config_key"));
    assert_eq!(
        unknown["error"]["data"]["problems"][0]["suggest"],
        "ui.language"
    );
    let invalid = set(
        &mut client,
        "c2",
        json!({"layer": "personal", "changes": [
            {"key": "ui.language", "input": "en"},
            {"key": "permission.start_read_only", "input": "yes"},
            {"key": "log.level", "value": "debug"},
        ]}),
    )
    .await;
    assert_eq!(reason(&invalid), Some("config_invalid"), "{invalid}");
    assert_eq!(invalid["error"]["message"], "配置有误，未保存。");
    let problems = invalid["error"]["data"]["problems"]
        .as_array()
        .expect("是数组");
    let codes: Vec<&str> = problems
        .iter()
        .map(|p| p["code"].as_str().unwrap())
        .collect();
    assert_eq!(codes, ["wrong_type", "wrong_layer"]);
    assert_eq!(
        problems[0]["message"],
        "permission.start_read_only 要写 true 或 false，写的是 yes。改成 permission.start_read_only = true。"
    );
    assert!(problems[0].get("line").is_none() && problems[0].get("file").is_none());
    assert_eq!(
        read(&home, PERSONAL).as_deref(),
        Some("[ui]\nlanguage = \"zh\"\n"),
        "一项都没收"
    );
}

#[tokio::test]
async fn a_value_not_in_the_options_is_refused() {
    let home = Home::new();
    let mut client = client(&home).await;
    let reply = set(
        &mut client,
        "c1",
        json!({"layer": "personal", "changes": [{"key": "ui.language", "value": "cn"}]}),
    )
    .await;
    assert_eq!(reason(&reply), Some("config_invalid"));
    assert_eq!(
        reply["error"]["data"]["problems"][0]["code"],
        "not_an_option"
    );
    assert_eq!(reply["error"]["data"]["problems"][0]["got"], "\"cn\"");
}

#[tokio::test]
async fn a_file_that_cannot_be_read_cannot_have_one_item_changed() {
    let home = Home::new();
    home.write(PERSONAL, "[ui\nlanguage = \"en\"\n");
    let mut client = client(&home).await;
    let reply = set(
        &mut client,
        "c1",
        json!({"layer": "personal", "changes": [{"key": "ui.language", "input": "zh"}]}),
    )
    .await;
    assert_eq!(reason(&reply), Some("config_file_broken"));
    let problem = &reply["error"]["data"]["problems"][0];
    assert_eq!(
        (problem["code"].clone(), problem["file"].clone()),
        (json!("syntax"), json!(PERSONAL))
    );
    assert_eq!(
        read(&home, PERSONAL).as_deref(),
        Some("[ui\nlanguage = \"en\"\n")
    );
    let whole = set(
        &mut client,
        "c2",
        json!({"layer": "personal", "text": "[ui]\nlanguage = \"zh\"\n",
               "version": version(b"[ui\nlanguage = \"en\"\n")}),
    )
    .await;
    assert!(
        whole["result"].is_object(),
        "整份换的不管读不读得进来：{whole}"
    );
    assert_eq!(
        read(&home, PERSONAL).as_deref(),
        Some("[ui]\nlanguage = \"zh\"\n")
    );
}

#[tokio::test]
async fn expect_guards_against_a_change_made_elsewhere() {
    let home = Home::new();
    home.write(PERSONAL, "[ui]\nlanguage = \"en\"\n");
    let mut client = client(&home).await;
    let stale = set(
        &mut client,
        "c1",
        json!({"layer": "personal", "changes": [
            {"key": "ui.language", "value": "zh", "expect": {"value": "ja"}},
        ]}),
    )
    .await;
    assert_eq!(reason(&stale), Some("config_conflict"));
    assert_eq!(stale["error"]["data"]["current"], json!({"value": "en"}));
    let unwritten = set(
        &mut client,
        "c2",
        json!({"layer": "personal", "changes": [
            {"key": "ui.language", "value": "zh", "expect": {"value": "en"}},
            {"key": "permission.start_read_only", "value": true, "expect": {"value": false}},
        ]}),
    )
    .await;
    assert_eq!(
        unwritten["error"]["data"]["current"],
        json!({}),
        "没写的是 {{}}"
    );
    assert_eq!(
        read(&home, PERSONAL).as_deref(),
        Some("[ui]\nlanguage = \"en\"\n")
    );
    let fine = set(
        &mut client,
        "c3",
        json!({"layer": "personal", "changes": [
            {"key": "ui.language", "value": "zh", "expect": {"value": "en"}},
            {"key": "permission.start_read_only", "value": true, "expect": {}},
        ]}),
    )
    .await;
    assert!(fine["result"].is_object(), "{fine}");
}

/// 小数的项照数值比 `expect`（施工 8-3 再补，2026-10-07 网页撞见）：文件里是 `1.0`，头发来的是 `1`（JS 的头写不出 `1.0`），
/// 算对得上；数值真不一样的照旧 `config_conflict`。
#[tokio::test]
async fn expect_on_a_float_compares_the_number() {
    let home = Home::new();
    home.write(
        PERSONAL,
        "[providers.dev.models.m-1.price]\ninput = 1.0\noutput = 2.5\n",
    );
    // 价格那几项在模型的清单里：这一条另外登记上。
    let items = [
        miyu_models::settings::ProviderSettings::ITEMS,
        miyu_models::settings::ModelSettings::ITEMS,
        miyu_models::settings::PriceSettings::ITEMS,
    ]
    .concat();
    let mut client = Client::connect(home.core_with_items(&Script::new([]), None, &[], &items));
    client.hello().await;
    let key = |name: &str| format!("providers.dev.models.m-1.price.{name}");
    let stale = set(
        &mut client,
        "c1",
        json!({"layer": "personal", "changes": [
            {"key": key("output"), "unset": true, "expect": {"value": 2}},
        ]}),
    )
    .await;
    assert_eq!(reason(&stale), Some("config_conflict"), "2.5 和 2 不一样");
    let fine = set(
        &mut client,
        "c2",
        json!({"layer": "personal", "changes": [
            {"key": key("input"), "unset": true, "expect": {"value": 1}},
            {"key": key("output"), "value": 3, "expect": {"value": 2.5}},
        ]}),
    )
    .await;
    assert!(fine["result"].is_object(), "1 就是 1.0：{fine}");
    let again = set(
        &mut client,
        "c3",
        json!({"layer": "personal", "changes": [
            {"key": key("output"), "value": 4, "expect": {"value": 3.0}},
        ]}),
    )
    .await;
    assert!(
        again["result"].is_object(),
        "写进去的 3 和 3.0 对得上：{again}"
    );
}

#[tokio::test]
async fn a_whole_new_text_needs_the_version_it_was_read_at_and_no_errors() {
    let home = Home::new();
    home.write(PERSONAL, "[ui]\nlanguage = \"en\"\n");
    let mut client = client(&home).await;
    let stale = set(
        &mut client,
        "c1",
        json!({"layer": "personal", "text": "[ui]\nlanguage = \"zh\"\n", "version": null}),
    )
    .await;
    assert_eq!(reason(&stale), Some("config_conflict"));
    assert_eq!(
        stale["error"]["data"]["version"],
        json!(version(b"[ui]\nlanguage = \"en\"\n"))
    );
    let wrong = set(
        &mut client,
        "c2",
        json!({"layer": "personal", "text": "[ui]\nlanguage = \"cn\"\n",
               "version": version(b"[ui]\nlanguage = \"en\"\n")}),
    )
    .await;
    assert_eq!(reason(&wrong), Some("config_invalid"));
    assert_eq!(wrong["error"]["data"]["problems"][0]["line"], 2);
    let warned = "# 换了\n[ui]\nlanguage = \"zh\"\nlangauge = 1\n";
    let saved = set(
        &mut client,
        "edit-1",
        json!({"layer": "personal", "text": warned, "version": version(b"[ui]\nlanguage = \"en\"\n")}),
    )
    .await;
    assert_eq!(
        saved["result"]["keys"]["ui.language"]["value"], "zh",
        "只有警告的照样存"
    );
    assert_eq!(read(&home, PERSONAL).as_deref(), Some(warned));
    let logged = journal(&home, "home/alice/journal.jsonl");
    assert_eq!(logged[0]["body"]["via"], "edit");
}

#[tokio::test]
async fn a_hand_edit_is_read_again_before_the_change() {
    let home = Home::new();
    home.write(PERSONAL, "[ui]\nlanguage = \"en\"\n");
    let mut client = client(&home).await;
    // 核心起来以后有人手改：核心手里的还是原来那一份。
    let hand = "\u{FEFF}# 手改的\r\n[ui]\r\nlanguage = \"ja\"\r\n";
    home.write(PERSONAL, hand);
    let reply = set(
        &mut client,
        "c1",
        json!({"layer": "personal", "changes": [{"key": "permission.start_read_only", "input": "true"}]}),
    )
    .await;
    assert!(reply["result"].is_object(), "{reply}");
    assert_eq!(
        read(&home, PERSONAL).as_deref(),
        Some(
            "\u{FEFF}# 手改的\r\n[ui]\r\nlanguage = \"ja\"\r\n\r\n[permission]\r\nstart_read_only = true\r\n"
        ),
        "在新的字上改，BOM、换行照原文件"
    );
    // 手改的先当手改记一条（`via: file`，施工 8-4），这一次改的只记它自己的那一项。
    let logged = journal(&home, "home/alice/journal.jsonl");
    assert_eq!(logged.len(), 2, "{logged:?}");
    assert_eq!(logged[0]["body"]["via"], "file");
    assert_eq!(
        logged[0]["body"]["changes"],
        json!([{"key": "ui.language", "old": "en", "new": "ja"}])
    );
    assert_eq!(
        logged[1]["body"]["changes"],
        json!([{"key": "permission.start_read_only", "new": true}]),
        "手改的那一项不算这一次改的"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_write_that_fails_changes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let home = Home::new();
    home.write(PERSONAL, "[ui]\nlanguage = \"en\"\n");
    let dir = home.root.path().join("home").join("alice");
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).expect("改得了");
    let mut client = client(&home).await;
    let reply = set(
        &mut client,
        "c1",
        json!({"layer": "personal", "changes": [{"key": "ui.language", "input": "zh"}]}),
    )
    .await;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).expect("改得了");
    assert_eq!(reason(&reply), Some("internal_error"), "{reply}");
    assert_eq!(
        read(&home, PERSONAL).as_deref(),
        Some("[ui]\nlanguage = \"en\"\n")
    );
    let names: Vec<String> = std::fs::read_dir(&dir)
        .expect("读得了")
        .map(|entry| {
            entry
                .expect("在")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, ["settings.toml"], "没留下临时文件、没记日志");
    let again = set(
        &mut client,
        "c2",
        json!({"layer": "personal", "changes": [{"key": "ui.language", "input": "en"}]}),
    )
    .await;
    assert_eq!(again["result"]["keys"], json!({}), "手里的还是原来那一份");
}

//! 统一检查（施工 8-30，`docs/blueprint/protocol.md` 的 `check`）：真核心走一遍。不写文件的查全部：配置照磁盘上现在的字、
//! 密钥文件照核心手里的、人格每一层各查各的；写了文件的只查那一份，认不出的 `unknown_file`；给人看的那一句照连接的语言。

mod support;

use serde_json::{Value, json};

use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use support::venues::configured_core;
use support::*;

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(configured_core(home, &Script::new([]), Catalog::default()));
    client.hello().await;
    client
}

/// 一处的几格：种类、文件、代码、级别、第几行。
fn brief(problem: &Value) -> (String, String, String, String, Option<u64>) {
    let text = |key: &str| problem[key].as_str().unwrap_or_default().to_string();
    (
        text("kind"),
        text("file"),
        text("code"),
        text("level"),
        problem["line"].as_u64(),
    )
}

#[tokio::test]
async fn everything_is_checked_from_disk_layer_by_layer() {
    let home = Home::new();
    let mut client = connected(&home).await;
    // 什么都还没写：还没有的文件跳过，一处都没有（出厂的软件工程师也没错）。
    let empty = client.call("k0", "check", json!({})).await;
    assert_eq!(empty["result"], json!({"problems": []}), "{empty}");
    home.write("system/secrets.toml", "DeepSeek = \"sk-x\"\n");
    let mut client = connected(&home).await;
    // 核心起来以后才改的：照磁盘上现在的查。
    home.write("system/config.toml", "[ui]\nlanguage = \"klingon\"\n");
    home.write("home/alice/settings.toml", "x.y = 1\n");
    home.write("home/alice/personas/miyu/persona.toml", "[voice]\n");
    home.write(
        "home/alice/personas/miyu/prompts/examples.md",
        "user: a\nuser: b\n",
    );
    home.write("system/personas/miyu/prompts/examples.md", "user: a\n");
    let reply = client.call("k1", "check", json!({})).await;
    let problems = reply["result"]["problems"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    // 写了 `cwd` 的连它的项目配置一起查，排在个人设置后面。
    let project = home.work.join(".miyu").join("config.toml");
    std::fs::create_dir_all(project.parent().unwrap()).unwrap();
    std::fs::write(&project, "[ui]\nlanguage = \"zh\"\n").unwrap();
    let with_cwd = client
        .call("k2", "check", json!({"cwd": home.work.to_string_lossy()}))
        .await;
    let kinds: Vec<_> = with_cwd["result"]["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(brief)
        .map(|(kind, _, code, _, _)| (kind, code))
        .collect();
    assert_eq!(
        kinds[2],
        ("config".to_string(), "wrong_layer".to_string()),
        "{with_cwd}"
    );
    assert_eq!(kinds.len(), problems.len() + 1);
    let seen: Vec<_> = problems.iter().map(brief).collect();
    assert_eq!(
        seen,
        [
            (
                "config".into(),
                "system/config.toml".into(),
                "not_an_option".into(),
                "error".into(),
                Some(2)
            ),
            (
                "config".into(),
                "home/alice/settings.toml".into(),
                "unknown_key".into(),
                "warning".into(),
                Some(1)
            ),
            (
                "secrets".into(),
                "system/secrets.toml".into(),
                "bad_format".into(),
                "error".into(),
                Some(1)
            ),
            (
                "persona".into(),
                "system/personas/miyu/prompts/examples.md".into(),
                "last_line".into(),
                "error".into(),
                Some(1)
            ),
            (
                "persona".into(),
                "home/alice/personas/miyu/persona.toml".into(),
                "unknown_table".into(),
                "error".into(),
                Some(1)
            ),
            (
                "persona".into(),
                "home/alice/personas/miyu/prompts/examples.md".into(),
                "take_turns".into(),
                "error".into(),
                Some(2)
            ),
        ],
        "{reply}"
    );
    let unknown_table = &problems[4]["message"];
    assert_eq!(
        unknown_table, "不认识的表 [voice]：persona.toml 里只能有 [persona]",
        "照连接的语言（握手报的中文）"
    );
}

#[tokio::test]
async fn one_file_is_checked_by_where_it_is() {
    let home = Home::new();
    let mut client = connected(&home).await;
    home.write(
        "home/alice/settings.toml",
        "[external.bindings]\n\"qq:1\" = \"alice\"\n",
    );
    home.write("home/alice/personas/miyu/prompts/examples.md", "user: a\n");
    home.write("system/config.toml", "[ui]\nlanguage = \"klingon\"\n");
    let root = home.root.path();
    let path = |relative: &str| root.join(relative).to_string_lossy().into_owned();
    let personal = client
        .call(
            "k1",
            "check",
            json!({"file": path("home/alice/settings.toml")}),
        )
        .await;
    let seen: Vec<_> = personal["result"]["problems"]
        .as_array()
        .unwrap()
        .iter()
        .map(brief)
        .collect();
    assert_eq!(seen.len(), 1, "只查那一份：{personal}");
    assert_eq!(seen[0].2, "wrong_layer", "{personal}");
    let examples = client
        .call(
            "k2",
            "check",
            json!({"file": path("home/alice/personas/miyu/prompts/examples.md")}),
        )
        .await;
    assert_eq!(
        examples["result"]["problems"][0]["code"], "last_line",
        "{examples}"
    );
    // 写了文件、还没有的：读不了。
    let missing = client
        .call(
            "k3",
            "check",
            json!({"file": path("home/alice/personas/new/persona.toml")}),
        )
        .await;
    assert_eq!(
        missing["result"]["problems"][0]["code"], "unreadable",
        "{missing}"
    );
    // 经链接写的、人格目录还没建的，照样认得（macOS 的 `/var` 是链接，CI 撞见过）。
    #[cfg(unix)]
    {
        let link = home.work.join("root-link");
        std::os::unix::fs::symlink(root, &link).unwrap();
        let through = link.join("home/alice/personas/fresh/persona.toml");
        let reply = client
            .call("k3b", "check", json!({"file": through.to_string_lossy()}))
            .await;
        assert_eq!(
            reply["result"]["problems"][0]["code"], "unreadable",
            "{reply}"
        );
    }
    // 项目配置照 `.miyu/config.toml` 认，不在 `cwd` 下面也行。
    let project = home.work.join(".miyu").join("config.toml");
    std::fs::create_dir_all(project.parent().unwrap()).unwrap();
    std::fs::write(&project, "[ui]\nlanguage = \"zh\"\n").unwrap();
    let checked = client
        .call("k4", "check", json!({"file": project.to_string_lossy()}))
        .await;
    assert_eq!(
        checked["result"]["problems"][0]["code"], "wrong_layer",
        "{checked}"
    );
    // 相对的照 `cwd` 接。
    let relative = client
        .call(
            "k5",
            "check",
            json!({"cwd": root.join("system").to_string_lossy(), "file": "config.toml"}),
        )
        .await;
    assert_eq!(
        relative["result"]["problems"][0]["code"], "not_an_option",
        "{relative}"
    );
    // 叫 config.toml、却不在 `.miyu` 下面的，认不出。
    let loose = home.work.join("config.toml");
    std::fs::write(&loose, "x = 1\n").unwrap();
    let reply = client
        .call("k8", "check", json!({"file": loose.to_string_lossy()}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_file"), "{reply}");
    let other = std::env::temp_dir().join("miyu-check-elsewhere.toml");
    let reply = client
        .call("k6", "check", json!({"file": other.to_string_lossy()}))
        .await;
    assert_eq!(reason(&reply), Some("unknown_file"), "{reply}");
    let reply = client.call("k7", "check", json!({"extra": 1})).await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

//! 吉祥物包（施工 F-7，`docs/blueprint/packages.md`「吉祥物包」）：真核心走一遍。清单只有 `[package]`、`[mascot]`，不说协议；
//! `package.list` 带 `mascot`，`package.file` 只给模型那一份，`miyu check` 查模型文件在、不超过 256 KiB、是 JSON 的对象；
//! 没有开关。

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::support::*;
use miyu_session::testkit::Script;

/// 吉祥物包的清单。
const PUDDING: &str = "[package]\nkind = \"mascot\"\nname = { zh = \"布丁\", en = \"Pudding\" }\n\n[mascot]\nmodel = \"art/mascot.json\"\n";

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

/// `miyu check` 报的吉祥物那几条：代码和说法。
fn mascot_problems(reply: &Value) -> Vec<(String, String)> {
    reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|problem| {
            problem["code"]
                .as_str()
                .is_some_and(|code| code.starts_with("mascot_"))
        })
        .map(|problem| {
            (
                problem["code"].as_str().unwrap_or_default().to_string(),
                problem["message"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

#[tokio::test]
async fn a_mascot_is_listed_and_its_model_is_read() {
    let home = Home::new();
    home.write("home/alice/packages/pudding/package.toml", PUDDING);
    home.write(
        "home/alice/packages/pudding/art/mascot.json",
        "{\"frames\": []}",
    );
    home.write("home/alice/packages/pudding/notes.txt", "not the model");
    let mut client = connected(&home).await;
    let listed = client.call("l", "package.list", json!({})).await;
    let pudding = listed["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .find(|item| item["package"] == "pudding")
        .cloned()
        .unwrap_or_else(|| panic!("列出来了：{listed}"));
    assert_eq!(pudding["kind"], "mascot");
    assert_eq!(pudding["name"], "布丁");
    assert_eq!(pudding["mascot"], json!({"model": "art/mascot.json"}));
    assert!(pudding.get("protocol").is_none(), "不说协议：{pudding}");

    let model = client
        .call(
            "f",
            "package.file",
            json!({"package": "pudding", "path": "art/mascot.json"}),
        )
        .await;
    let data = STANDARD
        .decode(
            model["result"]["data"]
                .as_str()
                .unwrap_or_else(|| panic!("{model}")),
        )
        .expect("是 base64");
    assert_eq!(data, b"{\"frames\": []}");
    for path in ["notes.txt", "", "art/../art/mascot.json"] {
        let other = client
            .call(
                "o",
                "package.file",
                json!({"package": "pudding", "path": path}),
            )
            .await;
        assert!(
            other.get("error").is_some(),
            "只给模型那一份：{path:?} {other}"
        );
    }
    let switched = client
        .call("e", "package.disable", json!({"package": "pudding"}))
        .await;
    assert_eq!(
        switched["error"]["data"]["reason"], "not_switchable",
        "{switched}"
    );
}

#[tokio::test]
async fn check_says_when_the_model_is_missing_too_big_or_not_an_object() {
    let home = Home::new();
    home.write("home/alice/packages/pudding/package.toml", PUDDING);
    let mut client = connected(&home).await;
    let model = "home/alice/packages/pudding/art/mascot.json";
    let reply = client.call("c1", "check", json!({})).await;
    assert_eq!(
        mascot_problems(&reply),
        [(
            "mascot_missing".to_string(),
            "包目录里没有 art/mascot.json".to_string()
        )],
        "{reply}"
    );
    home.write(model, "[1, 2]");
    let reply = client.call("c2", "check", json!({})).await;
    assert_eq!(
        mascot_problems(&reply),
        [(
            "mascot_not_json".to_string(),
            "art/mascot.json 不是 JSON 对象".to_string()
        )]
    );
    home.write(model, &format!("{{\"x\": \"{}\"}}", "a".repeat(300 * 1024)));
    let reply = client.call("c3", "check", json!({})).await;
    assert_eq!(
        mascot_problems(&reply),
        [(
            "mascot_too_big".to_string(),
            "art/mascot.json 超过 256 KiB".to_string()
        )]
    );
    home.write(model, "{\"frames\": []}");
    let reply = client.call("c4", "check", json!({})).await;
    assert!(mascot_problems(&reply).is_empty(), "{reply}");
}

#[tokio::test]
async fn a_mascot_takes_no_protocol_and_no_other_tables() {
    let home = Home::new();
    home.write(
        "home/alice/packages/a/package.toml",
        &PUDDING.replace("[package]\n", "[package]\nprotocol = [1, 1]\n"),
    );
    home.write(
        "home/alice/packages/b/package.toml",
        &format!("{PUDDING}\n[command]\nname = \"b\"\nprogram = \"b\"\n"),
    );
    home.write(
        "home/alice/packages/c/package.toml",
        &PUDDING.replace("art/mascot.json", "../mascot.json"),
    );
    home.write(
        "home/alice/packages/d/package.toml",
        "[package]\nkind = \"mascot\"\nname = { en = \"D\" }\n",
    );
    let mut client = connected(&home).await;
    let reply = client.call("c", "check", json!({})).await;
    let codes: Vec<(String, String)> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|problem| problem["kind"] == "package")
        .map(|problem| {
            (
                problem["file"].as_str().unwrap_or_default().to_string(),
                problem["code"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        codes,
        [
            (
                "home/alice/packages/a/package.toml".to_string(),
                "wrong_kind".to_string()
            ),
            (
                "home/alice/packages/b/package.toml".to_string(),
                "wrong_kind".to_string()
            ),
            (
                "home/alice/packages/c/package.toml".to_string(),
                "bad_mascot_model".to_string()
            ),
            (
                "home/alice/packages/d/package.toml".to_string(),
                "missing_key".to_string()
            ),
        ],
        "{reply}"
    );
}

//! 新建、改、删预设（施工 P-3 中，`docs/blueprint/presets.md`「改」）：真核心走一遍。只写家目录那一层；对没有的编号写就是新建，
//! 改出厂的只写改了的项；原来的注释、顺序照原样；有错、`expect` 对不上的整条不收、什么都不写；删掉家目录那一层回到下面的。

use std::sync::Arc;

use serde_json::{Value, json};

use crate::support::*;
use miyu_endpoint::Core;
use miyu_session::testkit::Script;

fn core(home: &Home) -> Arc<Core> {
    home.core(&Script::new([]))
}

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(core(home));
    client.hello().await;
    client
}

/// 家目录那一层的文件，没有的是空的。
fn mine(home: &Home, id: &str) -> Option<String> {
    std::fs::read_to_string(
        home.root
            .path()
            .join(format!("home/alice/presets/{id}.toml")),
    )
    .ok()
}

async fn set(client: &mut Client, id: &str, preset: &str, changes: Value) -> Value {
    client
        .call(
            id,
            "preset.set",
            json!({"preset": preset, "changes": changes}),
        )
        .await
}

#[tokio::test]
async fn a_new_id_is_made_in_one_call_and_a_session_can_use_it() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let made = set(
        &mut client,
        "s1",
        "mine",
        json!([
            {"key": "preset.base", "value": "dev"},
            {"key": "preset.name.zh", "value": "我的"},
            {"key": "software.memory", "value": true},
        ]),
    )
    .await;
    let made = &made["result"];
    assert_eq!(made["preset"], "mine", "{made}");
    assert_eq!(made["base"], "dev");
    assert_eq!(made["layers"], json!(["home"]));
    assert_eq!(
        (&made["name"]["zh"], &made["name"]["en"]),
        (&json!("我的"), &json!("Dev"))
    );
    assert_eq!(made["software"]["memory"], true);
    assert_eq!(
        mine(&home, "mine").as_deref(),
        Some(
            "[preset]\nbase = \"dev\"\n\n[preset.name]\nzh = \"我的\"\n\n[software]\nmemory = true\n"
        )
    );
    let created = client
        .call(
            "c1",
            "session.create",
            json!({"cwd": home.work.to_string_lossy(), "preset": "mine"}),
        )
        .await;
    assert!(created["result"]["session"].is_string(), "{created}");
}

#[tokio::test]
async fn changing_a_shipped_one_writes_only_what_changed_and_keeps_the_rest_of_the_file() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let got = set(
        &mut client,
        "s1",
        "dev",
        json!([{"key": "software.memory", "value": true}]),
    )
    .await;
    assert_eq!(got["result"]["layers"], json!(["shipped", "home"]), "{got}");
    assert_eq!(got["result"]["software"]["memory"], true);
    assert_eq!(
        mine(&home, "dev").as_deref(),
        Some("[software]\nmemory = true\n")
    );

    home.write(
        "home/alice/presets/full.toml",
        "# 我自己的\n[tools]\nshell = false # 先关着\n\n[preset]\nname = { zh = \"全开\" }\n",
    );
    set(
        &mut client,
        "s2",
        "full",
        json!([
            {"key": "preset.name.en", "value": "All"},
            {"key": "tools.trash", "value": false},
        ]),
    )
    .await;
    assert_eq!(
        mine(&home, "full").as_deref(),
        Some(
            "# 我自己的\n[tools]\nshell = false # 先关着\ntrash = false\n\n[preset]\nname = { zh = \"全开\", en = \"All\" }\n"
        ),
        "注释、顺序、行内表的写法照原样"
    );

    // 删一项：回到下面那一层的。
    let unset = set(
        &mut client,
        "s3",
        "dev",
        json!([{"key": "software.memory", "unset": true}]),
    )
    .await;
    assert_eq!(unset["result"]["software"].get("memory"), None, "{unset}");
    assert_eq!(
        unset["result"]["layers"],
        json!(["shipped", "home"]),
        "空了的文件照样是一层"
    );
}

#[tokio::test]
async fn conflicts_and_mistakes_write_nothing() {
    let home = Home::new();
    let mut client = connected(&home).await;
    set(
        &mut client,
        "s0",
        "dev",
        json!([{"key": "software.memory", "value": true}]),
    )
    .await;
    for (n, (expect, current)) in [
        (json!({"value": false}), json!({"value": true})),
        (json!({}), json!({"value": true})),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = set(
            &mut client,
            &format!("e{n}"),
            "dev",
            json!([{"key": "software.memory", "value": false, "expect": expect}]),
        )
        .await;
        assert_eq!(reason(&reply), Some("preset_conflict"), "{reply}");
        assert_eq!(reply["error"]["data"]["current"], current);
    }
    let matching = set(
        &mut client,
        "e9",
        "dev",
        json!([{"key": "software.net", "value": false, "expect": {}}]),
    )
    .await;
    assert_eq!(
        matching["result"]["software"]["net"], false,
        "对得上的照写：{matching}"
    );
    let before = mine(&home, "dev");

    for (n, (changes, problem)) in [
        (
            json!([{"key": "software.memory", "value": "yes"}]),
            "home dev.toml:2: software.memory must be true or false",
        ),
        (
            json!([{"key": "preset.colour", "value": "red"}]),
            "home dev.toml:",
        ),
        (
            json!([{"key": "preset.base", "value": "dev"}]),
            "base cycle: dev -> dev",
        ),
        (
            json!([{"key": "preset.base", "value": "nowhere"}]),
            r#"base "nowhere" of "dev" not found"#,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = set(&mut client, &format!("i{n}"), "dev", changes.clone()).await;
        assert_eq!(reason(&reply), Some("preset_invalid"), "{changes}：{reply}");
        let said = reply["error"]["data"]["problem"]
            .as_str()
            .unwrap_or_default();
        assert!(said.starts_with(problem), "{said}");
        assert_eq!(mine(&home, "dev"), before, "什么都没写：{changes}");
    }

    for (n, params) in [
        json!({"preset": "dev", "changes": []}),
        json!({"preset": "dev", "changes": [{"key": "software.net", "value": true, "unset": true}]}),
        json!({"preset": "dev", "changes": [{"key": "software.net"}]}),
        json!({"preset": "dev", "changes": [{"key": "software.net", "value": [true]}]}),
        json!({"preset": "dev", "changes": [{"key": "software.net", "unset": false}]}),
        json!({"preset": "dev", "changes": [{"key": "a", "value": 1}, {"key": "a", "value": 2}]}),
        json!({"preset": "dev", "changes": [{"key": "software.net", "value": true, "expect": {"was": 1}}]}),
        json!({"preset": "Not An Id", "changes": [{"key": "software.net", "value": true}]}),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = client.call(&format!("b{n}"), "preset.set", params.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{params}：{reply}");
    }
    assert_eq!(mine(&home, "dev"), before);
}

#[tokio::test]
async fn deleting_your_layer_goes_back_to_what_is_below() {
    let home = Home::new();
    let mut client = connected(&home).await;
    set(
        &mut client,
        "s1",
        "dev",
        json!([{"key": "software.memory", "value": true}]),
    )
    .await;
    set(
        &mut client,
        "s2",
        "mine",
        json!([{"key": "preset.base", "value": "full"}]),
    )
    .await;

    let back = client
        .call("d1", "preset.delete", json!({"preset": "dev"}))
        .await;
    assert_eq!(back["result"], json!({"remains": true}), "{back}");
    assert_eq!(mine(&home, "dev"), None);
    let got = client
        .call("g1", "preset.get", json!({"preset": "dev"}))
        .await;
    assert_eq!(got["result"]["layers"], json!(["shipped"]));

    let gone = client
        .call("d2", "preset.delete", json!({"preset": "mine"}))
        .await;
    assert_eq!(gone["result"], json!({"remains": false}), "{gone}");
    let got = client
        .call("g2", "preset.get", json!({"preset": "mine"}))
        .await;
    assert_eq!(reason(&got), Some("unknown_preset"), "{got}");

    for (n, id) in ["dev", "full", "nobody"].into_iter().enumerate() {
        let reply = client
            .call(&format!("n{n}"), "preset.delete", json!({"preset": id}))
            .await;
        assert_eq!(reason(&reply), Some("nothing_to_delete"), "{id}：{reply}");
    }
    let reply = client
        .call("x", "preset.delete", json!({"preset": "Bad"}))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

#[tokio::test]
async fn the_same_value_writes_nothing() {
    let home = Home::new();
    let mut client = connected(&home).await;
    set(
        &mut client,
        "s1",
        "dev",
        json!([{"key": "software.memory", "value": true}]),
    )
    .await;
    let path = home.root.path().join("home/alice/presets/dev.toml");
    let text = "[preset]\ndefault_persona = 'engineer'\n\n[software]\nmemory   =   true\n";
    std::fs::write(&path, text).expect("写得进");
    let again = set(
        &mut client,
        "s2",
        "dev",
        json!([
            {"key": "software.memory", "value": true},
            {"key": "preset.default_persona", "value": "engineer"},
            {"key": "tools.shell", "unset": true},
        ]),
    )
    .await;
    assert_eq!(again["result"]["software"]["memory"], true, "{again}");
    assert_eq!(
        std::fs::read_to_string(&path).expect("在"),
        text,
        "这一层本来就是这个值的不动，单引号也不换"
    );
    // 还没有的编号上只删不写：什么都不建。
    let ghost = set(
        &mut client,
        "s3",
        "ghost",
        json!([{"key": "software.net", "unset": true}]),
    )
    .await;
    assert_eq!(reason(&ghost), Some("unknown_preset"), "{ghost}");
    assert_eq!(mine(&home, "ghost"), None);
}

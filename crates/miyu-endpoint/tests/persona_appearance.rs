//! 人格的主题色、背景图（施工 P-6，`docs/blueprint/personas.md`「主题色、背景图」）：真核心走一遍。主题色写进家目录那一层的
//! `persona.toml` 的 `[appearance] seed`，读出来一律小写；背景图照头像的办法，只是上限大一些。

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::attach::png;
use crate::support::*;
use miyu_session::testkit::Script;

async fn connected(home: &Home) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    client
}

/// 传一份内容，交回它的哈希。
async fn put(client: &mut Client, bytes: &[u8], name: &str) -> String {
    let reply = client
        .call(
            "p",
            "blob.put",
            json!({"data": STANDARD.encode(bytes), "name": name}),
        )
        .await;
    reply["result"]["blob"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"))
        .to_string()
}

async fn set(client: &mut Client, params: Value) -> Value {
    client.call("s", "persona.set", params).await
}

/// 系统区里的一个人格 `base`：主题色 `#112233`，一张背景图。
fn base(home: &Home, background: &[u8]) {
    home.write(
        "system/personas/base/persona.toml",
        "[persona]\nname = \"底\"\n\n[appearance]\nseed = \"#112233\"\n",
    );
    let path = home.root.path().join("system/personas/base/background.png");
    std::fs::write(path, background).expect("写得进");
}

/// 家目录那一层的 `persona.toml`。
fn mine(home: &Home) -> Option<String> {
    std::fs::read_to_string(
        home.root
            .path()
            .join("home/alice/personas/base/persona.toml"),
    )
    .ok()
}

#[tokio::test]
async fn the_seed_is_written_lowercase_and_unset_back_to_the_layer_below() {
    let home = Home::new();
    base(&home, &png(64, 64));
    let mut client = connected(&home).await;
    let got = client
        .call("g", "persona.get", json!({"persona": "base"}))
        .await;
    assert_eq!(got["result"]["seed"], "#112233", "{got}");

    // 只改主题色的不用写 changes、prompts；大写也认，写成小写。
    let changed = set(&mut client, json!({"persona": "base", "seed": "#3368C0"})).await;
    assert_eq!(changed["result"]["seed"], "#3368c0", "{changed}");
    assert_eq!(
        mine(&home).as_deref(),
        Some("[appearance]\nseed = \"#3368c0\"\n")
    );
    let listed = client.call("l", "persona.list", json!({})).await;
    let row = listed["result"]["personas"]
        .as_array()
        .expect("有")
        .iter()
        .find(|row| row["persona"] == "base")
        .cloned()
        .expect("列出来了");
    assert_eq!(row["seed"], "#3368c0", "{listed}");

    let unset = set(
        &mut client,
        json!({"persona": "base", "seed": {"unset": true}}),
    )
    .await;
    assert_eq!(
        unset["result"]["seed"], "#112233",
        "回到下面那一层的：{unset}"
    );
}

#[tokio::test]
async fn a_wrong_seed_is_refused_and_nothing_is_written() {
    let home = Home::new();
    base(&home, &png(64, 64));
    let mut client = connected(&home).await;
    for seed in [
        json!("blue"),
        json!("#3368c"),
        json!("3368c0"),
        json!(7),
        json!({"unset": false}),
        json!({"unset": true, "more": 1}),
    ] {
        let reply = set(
            &mut client,
            json!({"persona": "base", "seed": seed.clone()}),
        )
        .await;
        assert_eq!(
            reply["error"]["data"]["reason"], "bad_params",
            "{seed}：{reply}"
        );
    }
    let twice = set(
        &mut client,
        json!({"persona": "base", "seed": "#000000", "changes": [{"key": "appearance.seed", "value": "#ffffff"}]}),
    )
    .await;
    assert_eq!(twice["error"]["data"]["reason"], "bad_params", "{twice}");
    assert_eq!(mine(&home), None, "什么都没写");
}

#[tokio::test]
async fn a_new_persona_can_start_with_only_a_seed() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let made = set(&mut client, json!({"seed": "#ABCDEF"})).await;
    assert_eq!(made["result"]["seed"], "#abcdef", "{made}");
    assert_eq!(made["result"]["background"], Value::Null);
}

#[tokio::test]
async fn a_background_is_set_read_and_unset_back_to_the_layer_below() {
    let home = Home::new();
    let lower = png(64, 64);
    base(&home, &lower);
    let mut client = connected(&home).await;
    let before = client
        .call("g", "persona.get", json!({"persona": "base"}))
        .await;
    let lower_version = before["result"]["background"]
        .as_str()
        .unwrap_or_else(|| panic!("{before}"))
        .to_string();

    // 比头像的上限大、在背景图的上限以内。
    let wide = png(4096, 2304);
    let blob = put(&mut client, &wide, "wide.png").await;
    let changed = set(
        &mut client,
        json!({"persona": "base", "background": {"blob": blob}}),
    )
    .await;
    let version = changed["result"]["background"]
        .as_str()
        .unwrap_or_else(|| panic!("{changed}"))
        .to_string();
    assert_ne!(version, lower_version);
    assert_eq!(changed["result"]["avatar"], Value::Null, "头像不动");
    assert!(
        home.root
            .path()
            .join("home/alice/personas/base/background.png")
            .is_file()
    );
    let read = client
        .call("b", "persona.background", json!({"persona": "base"}))
        .await;
    assert_eq!(
        (
            read["result"]["background"].clone(),
            read["result"]["media_type"].clone()
        ),
        (json!(version), json!("image/png")),
        "{read}"
    );
    assert_eq!(
        STANDARD
            .decode(read["result"]["data"].as_str().expect("有图"))
            .expect("是 base64"),
        wide
    );

    let stale = set(
        &mut client,
        json!({"persona": "base", "background": {"unset": true, "expect": lower_version}}),
    )
    .await;
    assert_eq!(
        stale["error"]["data"]["reason"], "persona_conflict",
        "{stale}"
    );
    let unset = set(
        &mut client,
        json!({"persona": "base", "background": {"unset": true, "expect": version}}),
    )
    .await;
    assert_eq!(
        unset["result"]["background"],
        json!(lower_version),
        "{unset}"
    );
}

#[tokio::test]
async fn wrong_backgrounds_are_refused_and_nothing_is_written() {
    let home = Home::new();
    base(&home, &png(64, 64));
    let mut client = connected(&home).await;
    let text = put(&mut client, b"just words\n", "a.txt").await;
    let tall = put(&mut client, &png(10, 5000), "tall.png").await;
    for (background, reason) in [
        (json!({"blob": text}), "background_not_image"),
        (json!({"blob": tall}), "background_too_big"),
        (json!({}), "bad_params"),
    ] {
        let reply = set(
            &mut client,
            json!({"persona": "base", "seed": "#000000", "background": background.clone()}),
        )
        .await;
        assert_eq!(
            reply["error"]["data"]["reason"], reason,
            "{background}：{reply}"
        );
        if reason == "background_too_big" {
            assert_eq!(
                (
                    reply["error"]["data"]["height"].clone(),
                    reply["error"]["data"]["max_side"].clone(),
                    reply["error"]["data"]["max_bytes"].clone(),
                ),
                (json!(5000), json!(4096), json!(5 * 1024 * 1024))
            );
        }
    }
    assert!(
        !home.root.path().join("home/alice/personas/base").exists(),
        "图不合规矩，一起改的主题色也没写"
    );
    let none = client
        .call("n", "persona.background", json!({"persona": "nobody"}))
        .await;
    assert_eq!(none["error"]["data"]["reason"], "unknown_persona", "{none}");
}

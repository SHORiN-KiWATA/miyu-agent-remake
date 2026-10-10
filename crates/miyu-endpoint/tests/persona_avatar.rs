//! 人格的头像、预设的图标（施工 P-5，`docs/blueprint/personas.md`「头像」、`presets.md`「图标」）：真核心走一遍。头像先
//! `blob.put` 传上来，`persona.set` 交它的哈希，写进家目录那一层；`persona.avatar` 读回来；删了回到下面那一层的。

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

async fn set(client: &mut Client, persona: &str, avatar: Value) -> Value {
    client
        .call(
            "s",
            "persona.set",
            json!({"persona": persona, "avatar": avatar}),
        )
        .await
}

/// 系统区里的一个人格 `base`，带一张头像。
fn base(home: &Home, picture: &[u8]) {
    home.write(
        "system/personas/base/persona.toml",
        "[persona]\nname = \"底\"\n",
    );
    let path = home.root.path().join("system/personas/base/avatar.png");
    std::fs::write(path, picture).expect("写得进");
}

#[tokio::test]
async fn an_avatar_is_set_read_and_unset_back_to_the_layer_below() {
    let home = Home::new();
    let lower = png(32, 32);
    base(&home, &lower);
    let mut client = connected(&home).await;
    let before = client
        .call("g", "persona.get", json!({"persona": "base"}))
        .await;
    let lower_version = before["result"]["avatar"]
        .as_str()
        .expect("下面那一层有")
        .to_string();
    assert_eq!(lower_version.len(), 16);

    let mine = png(64, 48);
    let blob = put(&mut client, &mine, "me.png").await;
    let changed = set(&mut client, "base", json!({"blob": blob})).await;
    let version = changed["result"]["avatar"]
        .as_str()
        .unwrap_or_else(|| panic!("{changed}"))
        .to_string();
    assert_ne!(version, lower_version, "换上了家目录那一层的");
    assert!(
        home.root
            .path()
            .join("home/alice/personas/base/avatar.png")
            .is_file()
    );
    let read = client
        .call("a", "persona.avatar", json!({"persona": "base"}))
        .await;
    assert_eq!(
        (
            read["result"]["avatar"].clone(),
            read["result"]["media_type"].clone()
        ),
        (json!(version), json!("image/png"))
    );
    assert_eq!(
        STANDARD
            .decode(read["result"]["data"].as_str().expect("有图"))
            .expect("是 base64"),
        mine
    );
    let listed = client.call("l", "persona.list", json!({})).await;
    assert!(
        listed["result"]["personas"]
            .as_array()
            .expect("有")
            .contains(
                &json!({"persona": "base", "name": "底", "summary": null, "avatar": version, "background": null, "seed": null})
            ),
        "{listed}"
    );

    // 版本对不上的不动；对得上的照做。
    let stale = set(
        &mut client,
        "base",
        json!({"unset": true, "expect": lower_version}),
    )
    .await;
    assert_eq!(
        stale["error"]["data"]["reason"], "persona_conflict",
        "{stale}"
    );
    let unset = set(
        &mut client,
        "base",
        json!({"unset": true, "expect": version}),
    )
    .await;
    assert_eq!(unset["result"]["avatar"], json!(lower_version), "{unset}");
    assert!(
        !home
            .root
            .path()
            .join("home/alice/personas/base/avatar.png")
            .exists()
    );
}

#[tokio::test]
async fn a_new_persona_can_bring_its_avatar_and_one_without_has_none() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let blob = put(&mut client, &png(16, 16), "a.png").await;
    let made = client
        .call(
            "n",
            "persona.set",
            json!({"changes": [{"key": "persona.name", "value": "新的"}], "avatar": {"blob": blob}}),
        )
        .await;
    assert_eq!(
        made["result"]["avatar"].as_str().map(str::len),
        Some(16),
        "{made}"
    );
    let plain = client
        .call(
            "m",
            "persona.set",
            json!({"changes": [{"key": "persona.name", "value": "没头像"}]}),
        )
        .await;
    assert_eq!(plain["result"]["avatar"], Value::Null, "{plain}");
    let id = plain["result"]["persona"]
        .as_str()
        .expect("有编号")
        .to_string();
    let read = client
        .call("r", "persona.avatar", json!({"persona": id}))
        .await;
    assert_eq!(read["result"], Value::Null, "{read}");
}

#[tokio::test]
async fn wrong_avatars_are_refused_and_nothing_is_written() {
    let home = Home::new();
    base(&home, &png(32, 32));
    let mut client = connected(&home).await;
    let text = put(&mut client, b"just words\n", "a.txt").await;
    // GIF 认得出是图，可头像只收 PNG、JPEG、WebP。
    let gif = put(&mut client, b"GIF89a\x10\x00\x10\x00\x00\x00\x00;", "a.gif").await;
    let wide = put(&mut client, &png(2000, 10), "wide.png").await;
    let both = put(&mut client, &png(8, 8), "b.png").await;
    let missing = format!("sha256:{}", "0".repeat(64));
    for (avatar, reason) in [
        (json!({"blob": text}), "avatar_not_image"),
        (json!({"blob": gif}), "avatar_not_image"),
        (json!({"blob": wide}), "avatar_too_big"),
        (json!({"blob": missing}), "unknown_attachment"),
        (json!({"blob": both, "unset": true}), "bad_params"),
        (json!({}), "bad_params"),
        (json!({"unset": false}), "bad_params"),
    ] {
        let reply = set(&mut client, "base", avatar.clone()).await;
        assert_eq!(
            reply["error"]["data"]["reason"], reason,
            "{avatar}：{reply}"
        );
    }
    let wide_again = put(&mut client, &png(2000, 10), "w.png").await;
    let wide = set(&mut client, "base", json!({"blob": wide_again})).await;
    assert_eq!(
        (
            wide["error"]["data"]["width"].clone(),
            wide["error"]["data"]["max_side"].clone()
        ),
        (json!(2000), json!(1024))
    );
    assert!(
        !home.root.path().join("home/alice/personas/base").exists(),
        "什么都没写"
    );
    let unknown = client
        .call("u", "persona.avatar", json!({"persona": "nobody"}))
        .await;
    assert_eq!(
        unknown["error"]["data"]["reason"], "unknown_persona",
        "{unknown}"
    );
}

#[tokio::test]
async fn presets_carry_their_icons_and_one_can_be_changed() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let listed = client.call("l", "preset.list", json!({})).await;
    let icons: Vec<(String, Value)> = listed["result"]["presets"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .map(|one| {
            (
                one["preset"].as_str().unwrap_or_default().to_string(),
                one["icon"].clone(),
            )
        })
        .collect();
    assert_eq!(
        icons,
        [
            ("dev".to_string(), json!("box")),
            ("full".to_string(), json!("boxes"))
        ]
    );
    let changed = client
        .call(
            "s",
            "preset.set",
            json!({"preset": "dev", "changes": [{"key": "preset.icon", "value": "rocket"}]}),
        )
        .await;
    assert_eq!(changed["result"]["icon"], "rocket", "{changed}");
    let wrong = client
        .call(
            "w",
            "preset.set",
            json!({"preset": "dev", "changes": [{"key": "preset.icon", "value": "Rocket"}]}),
        )
        .await;
    assert_eq!(
        wrong["error"]["data"]["reason"], "preset_invalid",
        "{wrong}"
    );
    let back = client
        .call(
            "b",
            "preset.set",
            json!({"preset": "dev", "changes": [{"key": "preset.icon", "unset": true}]}),
        )
        .await;
    assert_eq!(back["result"]["icon"], "box", "回到出厂的：{back}");
}

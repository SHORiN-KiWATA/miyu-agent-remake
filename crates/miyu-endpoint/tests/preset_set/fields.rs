//! 预设文件里的几格怎么改（施工 P-4 上、F-3 下，从 `preset_set.rs` 挪出来：那边放不下了）：撤掉的默认人格不收；照功能改。

use serde_json::json;

use super::{connected, mine, on, set};
use crate::support::*;

/// 预设不再有默认人格（施工 P-4 上，2026-10-08 项目主人：只去掉预设的「默认人格」）：`preset.set` 写这个键参数不对，什么都不写；
/// 旧文件里写了的，下一次写这份文件时去掉；`preset.get` 没有这一格。
#[tokio::test]
async fn the_default_persona_is_gone_from_presets() {
    let home = Home::new();
    let mut client = connected(&home).await;
    for (n, changes) in [
        json!([{"key": "preset.default_persona", "value": "engineer"}]),
        json!([{"key": "preset.default_persona", "unset": true}]),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = set(&mut client, &format!("b{n}"), "dev", changes.clone()).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{changes}：{reply}");
    }
    assert_eq!(mine(&home, "dev"), None, "什么都没写");
    let dir = home.root.path().join("home/alice/presets");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("old.toml"),
        "[preset]\nname = \"我的\"\ndefault_persona = \"engineer\"\n",
    )
    .unwrap();
    let got = client
        .call("g", "preset.get", json!({"preset": "old"}))
        .await;
    assert!(got["result"].get("default_persona").is_none(), "{got}");
    let reply = set(
        &mut client,
        "s",
        "old",
        json!([{"key": "preset.summary", "value": "改过"}]),
    )
    .await;
    assert!(reply.get("error").is_none(), "{reply}");
    assert_eq!(
        mine(&home, "old").as_deref(),
        Some("[preset]\nname = \"我的\"\nsummary = \"改过\"\n")
    );
}

/// 照功能改（施工 F-3 下）：`features.<编号>` 写进 `[features]`，回应里那个功能跟着变，别的照旧。
#[tokio::test]
async fn a_feature_is_switched_by_its_id() {
    let home = Home::new();
    let mut client = connected(&home).await;
    let reply = set(
        &mut client,
        "f1",
        "full",
        json!([{"key": "features.commands", "value": false}]),
    )
    .await;
    assert_eq!(on(&reply, "commands"), Some(false), "{reply}");
    assert_eq!(on(&reply, "files"), Some(true));
    assert_eq!(
        mine(&home, "full").as_deref(),
        Some("[features]\ncommands = false\n")
    );
}

//! 桥用的配置（施工 O-20，`onebot.md` 第一条「怎么走」第 1 条、「施工时定的」第 38 条）：握手交来的 `config` 读成两个端口、
//! 令牌；没交的、`null` 的、不是 0 到 65535 的整数的端口照清单的默认值；令牌是字的照它（去掉前后空白），别的是没有；推来的
//! 只换带了的键，别的键不认。清单的默认值照出厂的清单读（8301、8302），读不出来的说是哪个文件。白名单成员（施工 O-23）：
//! `onebot.trusted` 字的列表照收，别的是空的。

use serde_json::{Map, Value, json};

use miyu_config::secret::Secret;
use miyu_onebot::settings::{Defaults, Settings, whitelist, whitelist_key};
use miyu_store::resources::ResourceRoot;

use crate::support::{TOKEN, defaults, resources};

/// 测试用的默认值：和出厂的不一样，看得出照的是它。
const FALLBACK: Defaults = Defaults {
    listen: 18301,
    web: 18302,
};

/// 令牌 `token`。
fn secret(token: &str) -> Option<Secret> {
    Some(Secret::new(token).expect("合写法"))
}

/// `keys` 当推来的变化。
fn keys(value: Value) -> Map<String, Value> {
    value.as_object().expect("是对象").clone()
}

#[test]
fn the_shipped_manifest_gives_the_two_default_ports() {
    assert_eq!(
        defaults(),
        Defaults {
            listen: 8301,
            web: 8302
        }
    );
}

#[test]
fn a_missing_manifest_or_default_is_named() {
    let dir = std::env::temp_dir().join(format!("miyu-onebot-defaults-{}", std::process::id()));
    let packages = dir.join("packages");
    std::fs::create_dir_all(&packages).expect("建得了");
    let error = Defaults::load(&ResourceRoot::at(dir.clone())).expect_err("没有清单");
    assert!(error.contains("onebot"), "{error}");
    // 照出厂的清单写一份，去掉 `onebot.web` 的默认值。
    let shipped = std::fs::read_to_string(resources().join("packages").join("onebot.toml"))
        .expect("读得到出厂的清单");
    let without = shipped.replace("default = 8302\n", "");
    assert_ne!(without, shipped, "真的去掉了");
    std::fs::write(packages.join("onebot.toml"), without).expect("写得进");
    let error = Defaults::load(&ResourceRoot::at(dir.clone())).expect_err("没写默认值");
    assert!(
        error.contains("onebot.toml") && error.contains("onebot.web"),
        "{error}"
    );
    std::fs::write(packages.join("onebot.toml"), "not toml [").expect("写得进");
    let error = Defaults::load(&ResourceRoot::at(dir.clone())).expect_err("读不成");
    assert!(error.contains("onebot.toml"), "{error}");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[test]
fn the_handed_config_is_taken_as_it_comes() {
    let handed = json!({"onebot.listen": 9000, "onebot.web": 9001, "onebot.token": TOKEN, "onebot.trusted": ["qq:20017"]});
    assert_eq!(
        Settings::handed(&handed, &FALLBACK),
        Settings {
            port: 9000,
            web: 9001,
            token: secret(TOKEN)
        }
    );
    assert_eq!(
        Settings::handed(&json!({"onebot.token": format!("  {TOKEN}\n")}), &FALLBACK).token,
        secret(TOKEN),
        "前后的空白去掉"
    );
    assert_eq!(
        Settings::handed(&json!({"onebot.listen": 0, "onebot.web": 65535}), &FALLBACK),
        Settings {
            port: 0,
            web: 65535,
            token: None
        },
        "0 到 65535 都照它"
    );
}

#[test]
fn what_is_not_handed_falls_back_to_the_manifest() {
    let nothing = Settings {
        port: 18301,
        web: 18302,
        token: None,
    };
    for config in [
        json!({}),
        Value::Null,
        json!("not an object"),
        json!({"onebot.listen": null, "onebot.web": null, "onebot.token": null}),
        json!({"onebot.listen": 65536, "onebot.web": -1, "onebot.token": ""}),
        json!({"onebot.listen": "8301", "onebot.web": 8302.5, "onebot.token": 42}),
        json!({"listen": 9000, "web.port": 9001, "onebotx.token": TOKEN, "onebot.tokens": TOKEN}),
    ] {
        assert_eq!(Settings::handed(&config, &FALLBACK), nothing, "{config}");
    }
}

#[test]
fn a_push_changes_only_the_keys_it_carries() {
    let mut settings = Settings::handed(
        &json!({"onebot.listen": 9000, "onebot.web": 9001, "onebot.token": TOKEN}),
        &FALLBACK,
    );
    settings.change(&keys(json!({"onebot.token": "new"})), &FALLBACK);
    assert_eq!(
        settings,
        Settings {
            port: 9000,
            web: 9001,
            token: secret("new")
        }
    );
    settings.change(&keys(json!({"onebot.listen": 9100})), &FALLBACK);
    assert_eq!((settings.port, settings.web), (9100, 9001));
    settings.change(
        &keys(json!({"onebot.web": null, "onebot.token": null})),
        &FALLBACK,
    );
    assert_eq!(
        settings,
        Settings {
            port: 9100,
            web: 18302,
            token: None
        },
        "null 当没有：端口照默认值，令牌没了"
    );
    settings.change(
        &keys(json!({"onebot.trusted": ["qq:1"], "web.port": 1})),
        &FALLBACK,
    );
    assert_eq!((settings.port, settings.web), (9100, 18302), "别的键不认");
}

#[test]
fn whitelist_is_a_list_of_text() {
    assert_eq!(whitelist_key(), "onebot.trusted");
    let handed = json!({"onebot.trusted": ["qq:20003", 7, null, "qq:20005"]});
    assert_eq!(
        whitelist(&handed[whitelist_key()]),
        ["qq:20003", "qq:20005"],
        "列表里不是字的不要"
    );
    for nothing in [
        json!({}),
        json!({"onebot.trusted": null}),
        json!({"onebot.trusted": "qq:1"}),
    ] {
        assert!(whitelist(&nothing[whitelist_key()]).is_empty(), "{nothing}");
    }
}

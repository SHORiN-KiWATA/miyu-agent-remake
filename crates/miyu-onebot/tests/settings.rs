//! 桥用的配置（施工 O-20，`onebot.md` 第一条「怎么走」第 1 条、「施工时定的」第 38 条）：握手交来的 `config` 读成 NapCat 的
//! 端口、令牌；没交的、`null` 的、不是 0 到 65535 的整数的端口照清单的默认值；令牌是字的照它（去掉前后空白），别的是没有；推来
//! 的只换带了的键，别的键不认。清单的默认值照出厂的清单读（8301），读不出来的说是哪个文件。白名单成员（施工 O-23）：
//! `onebot.whitelist`（施工 O-27 改名，旧键 `onebot.trusted` 不认）字的列表照收，别的是空的。原来桥自己的网页的端口
//! `onebot.web` 随施工 O-28 下去掉：桥不认，照出厂的清单拼的配置项里写它报不认识的键（「施工时定的」第 170 条）。

use serde_json::{Map, Value, json};

use miyu_config::secret::Secret;
use miyu_onebot::settings::{Defaults, Settings, whitelist, whitelist_key};
use miyu_store::resources::ResourceRoot;

use miyu_config::Layer;
use miyu_config::problem::Code;

use crate::support::{TOKEN, admin, defaults, resources, temp_root};

/// 测试用的默认值：和出厂的不一样，看得出照的是它。
const FALLBACK: Defaults = Defaults { listen: 18301 };

/// 令牌 `token`。
fn secret(token: &str) -> Option<Secret> {
    Some(Secret::new(token).expect("合写法"))
}

/// `keys` 当推来的变化。
fn keys(value: Value) -> Map<String, Value> {
    value.as_object().expect("是对象").clone()
}

#[test]
fn the_shipped_manifest_gives_the_default_port() {
    assert_eq!(defaults(), Defaults { listen: 8301 });
}

#[test]
fn a_missing_manifest_or_default_is_named() {
    let dir = std::env::temp_dir().join(format!("miyu-onebot-defaults-{}", std::process::id()));
    let packages = dir.join("packages");
    std::fs::create_dir_all(&packages).expect("建得了");
    let error = Defaults::load(&ResourceRoot::at(dir.clone())).expect_err("没有清单");
    assert!(error.contains("onebot"), "{error}");
    // 照出厂的清单写一份，去掉 `onebot.listen` 的默认值。
    let shipped = std::fs::read_to_string(resources().join("packages").join("onebot.toml"))
        .expect("读得到出厂的清单");
    let without = shipped.replace("default = 8301\n", "");
    assert_ne!(without, shipped, "真的去掉了");
    std::fs::write(packages.join("onebot.toml"), without).expect("写得进");
    let error = Defaults::load(&ResourceRoot::at(dir.clone())).expect_err("没写默认值");
    assert!(
        error.contains("onebot.toml") && error.contains("onebot.listen"),
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
    let handed =
        json!({"onebot.listen": 9000, "onebot.token": TOKEN, "onebot.whitelist": ["qq:20017"]});
    assert_eq!(
        Settings::handed(&handed, &FALLBACK),
        Settings {
            port: 9000,
            token: secret(TOKEN)
        }
    );
    assert_eq!(
        Settings::handed(&json!({"onebot.token": format!("  {TOKEN}\n")}), &FALLBACK).token,
        secret(TOKEN),
        "前后的空白去掉"
    );
    assert_eq!(
        Settings::handed(&json!({"onebot.listen": 0}), &FALLBACK).port,
        0,
        "0 也照它"
    );
    assert_eq!(
        Settings::handed(&json!({"onebot.listen": 65535}), &FALLBACK).port,
        65535,
        "65535 也照它"
    );
}

#[test]
fn what_is_not_handed_falls_back_to_the_manifest() {
    let nothing = Settings {
        port: 18301,
        token: None,
    };
    for config in [
        json!({}),
        Value::Null,
        json!("not an object"),
        json!({"onebot.listen": null, "onebot.token": null}),
        json!({"onebot.listen": 65536, "onebot.token": ""}),
        json!({"onebot.listen": -1}),
        json!({"onebot.listen": "8301", "onebot.token": 42}),
        json!({"onebot.listen": 8302.5}),
        json!({"listen": 9000, "web.port": 9001, "onebotx.token": TOKEN, "onebot.tokens": TOKEN}),
        // 原来桥自己的网页的端口（施工 O-28 下去掉）：不认。
        json!({"onebot.web": 9001}),
    ] {
        assert_eq!(Settings::handed(&config, &FALLBACK), nothing, "{config}");
    }
}

#[test]
fn a_push_changes_only_the_keys_it_carries() {
    let mut settings = Settings::handed(
        &json!({"onebot.listen": 9000, "onebot.token": TOKEN}),
        &FALLBACK,
    );
    settings.change(&keys(json!({"onebot.token": "new"})), &FALLBACK);
    assert_eq!(
        settings,
        Settings {
            port: 9000,
            token: secret("new")
        }
    );
    settings.change(&keys(json!({"onebot.listen": 9100})), &FALLBACK);
    assert_eq!(settings.port, 9100);
    settings.change(&keys(json!({"onebot.token": null})), &FALLBACK);
    assert_eq!(
        settings,
        Settings {
            port: 9100,
            token: None
        },
        "null 当没有：令牌没了"
    );
    settings.change(&keys(json!({"onebot.listen": null})), &FALLBACK);
    assert_eq!(settings.port, 18301, "null 当没有：端口照默认值");
    settings.change(
        &keys(json!({"onebot.whitelist": ["qq:1"], "web.port": 1, "onebot.web": 9001})),
        &FALLBACK,
    );
    assert_eq!(
        settings,
        Settings {
            port: 18301,
            token: None
        },
        "别的键不认"
    );
}

#[test]
fn whitelist_is_a_list_of_text() {
    assert_eq!(whitelist_key(), "onebot.whitelist");
    let handed = json!({"onebot.whitelist": ["qq:20003", 7, null, "qq:20005"]});
    assert_eq!(
        whitelist(&handed[whitelist_key()]),
        ["qq:20003", "qq:20005"],
        "列表里不是字的不要"
    );
    for nothing in [
        json!({}),
        json!({"onebot.whitelist": null}),
        json!({"onebot.whitelist": "qq:1"}),
        json!({"onebot.trusted": ["qq:20003"]}),
    ] {
        assert!(whitelist(&nothing[whitelist_key()]).is_empty(), "{nothing}");
    }
}

#[test]
fn the_web_port_is_an_unknown_key_now() {
    // 照真核心起来时那样把出厂的包的配置项拼进来（`Packaged`，同 `support::Home`）。包的程序不在主程序旁边的当没装、配置项不拼
    // （`package-pages.md`「程序不在就当没装」）：先把 `miyu-onebot` 链到测试程序旁边（同 `Home::spawning`）。
    crate::support::spawning::linked();
    let (dir, root) = temp_root();
    let shipped = ResourceRoot::at(resources());
    let mut found = miyu_endpoint::packages::load(&shipped, &root, &admin());
    let listed = miyu_core::settings::Packaged::of(&mut found).all();
    let codes = |source: &str| -> Vec<(Code, Option<String>)> {
        miyu_config::parse::parse(&listed, Layer::System, source)
            .expect("写法对")
            .problems
            .into_iter()
            .map(|problem| (problem.code, problem.key))
            .collect()
    };
    assert_eq!(
        codes("[onebot]\nlisten = 8301\n"),
        Vec::<(Code, Option<String>)>::new(),
        "NapCat 的端口照旧认"
    );
    assert_eq!(
        codes("[onebot]\nweb = 8302\n"),
        [(Code::UnknownKey, Some("onebot.web".to_string()))],
        "原来桥自己的网页的端口不认（施工 O-28 下）"
    );
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

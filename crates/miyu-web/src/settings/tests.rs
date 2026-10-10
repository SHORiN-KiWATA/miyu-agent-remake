//! 网页软件的数（施工 9-1 下）：默认值照清单 `packages/web/package.toml`，常量照 `web/web.json`；核心交回的最终值盖上去，不合范围的
//! 照旧。

use std::path::Path;

use serde_json::json;

use super::*;

fn shipped() -> Settings {
    Settings::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")).unwrap()
}

#[test]
fn the_defaults_come_from_the_manifest_and_the_constants_from_web_json() {
    let settings = shipped();
    assert_eq!(
        (
            settings.port,
            settings.idle_seconds,
            settings.ticket_idle_seconds,
            settings.most_tickets
        ),
        (8300, 600, 43_200, 4096)
    );
    assert!(settings.csp.starts_with("default-src 'self'"));
    assert_eq!(
        settings.types.get("html").map(String::as_str),
        Some("text/html; charset=utf-8")
    );
}

#[test]
fn final_values_from_the_core_win_and_odd_ones_are_ignored() {
    let items = json!({
        "web.port": {"value": 9000, "origin": {"layer": "system"}},
        "web.idle_seconds": {"value": 30},
        "web.most_tickets": {"value": -1},
    });
    let settings = shipped().configured(&items);
    assert_eq!(settings.port, 9000);
    assert_eq!(settings.idle_seconds, 30);
    assert_eq!(settings.ticket_idle_seconds, 43_200, "没交的照旧");
    assert_eq!(settings.most_tickets, 4096, "不合范围的照旧");
    assert_eq!(
        shipped()
            .configured(&json!({"web.port": {"value": 70000}}))
            .port,
        8300
    );
}

#[test]
fn every_key_asked_for_is_declared_in_the_manifest() {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../resources")
            .join(MANIFEST),
    )
    .unwrap();
    let manifest = miyu_config::package::read(&text).unwrap();
    let declared: Vec<String> = manifest
        .settings
        .iter()
        .map(|setting| format!("web.{}", setting.name))
        .collect();
    assert_eq!(declared, KEYS);
    let hidden: Vec<bool> = manifest
        .settings
        .iter()
        .map(|setting| setting.hidden)
        .collect();
    assert_eq!(hidden, [false, true, true, true], "端口露，别的不露");
}

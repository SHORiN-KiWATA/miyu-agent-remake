use serde_json::json;

use super::schema::Schema;
use super::{Entry, More};

fn schema(pages: &[&str]) -> Schema {
    let pages_json: Vec<_> = pages.iter().map(|p| json!({"id": p, "name": p})).collect();
    let groups: Vec<_> = pages
        .iter()
        .map(|p| json!({"id": format!("g-{p}"), "name": "组", "page": p}))
        .collect();
    let items: Vec<_> = pages
        .iter()
        .map(|p| json!({"key": format!("{p}.x"), "control": "text", "layers": ["personal"], "name": "x", "page": p, "group": format!("g-{p}")}))
        .collect();
    Schema::read(&json!({"pages": pages_json, "groups": groups, "items": items}))
}

#[test]
fn the_menu_puts_general_first_and_personas_before_advanced() {
    // 2026-10-07 项目主人：通用第一行，人格、预设放在下面。
    let mut more = More::default();
    assert_eq!(
        more.entries(),
        [Entry::Models, Entry::Personas],
        "没读到清单以前"
    );
    // 核心 F-4 再补、F-6 上起只给这几页：「权限」并进了「通用」，「接入」并进了「软件包」（2026-10-10 项目主人）。
    more.schema = Some(schema(&[
        "general", "models", "packages", "advanced", "voice",
    ]));
    assert_eq!(
        more.entries(),
        [
            Entry::Page("general".into()),
            Entry::Models,
            Entry::Personas,
            Entry::Page("packages".into()),
            Entry::Page("voice".into()),
            Entry::Page("advanced".into()),
        ]
    );
    more.menu_at = 99;
    assert_eq!(
        more.selected(),
        Entry::Page("advanced".into()),
        "越界停在最后"
    );
}

#[test]
fn a_removed_mascot_package_is_named_as_not_installed() {
    // 「吉祥物包」第 3 条（2026-10-11）：选着的吉祥物包卸掉了，值不写裸编号，写「pudding（未安装）」。
    let texts = crate::config::Config::builtin().unwrap().text.settings.more;
    let s = Schema::read(&json!({"items": [
        {"key": "tui.mascot", "control": "text", "layers": ["personal"], "name": "吉祥物", "page": "general", "group": "tui"}
    ]}));
    let item = &s.items[0];
    let installed = vec![crate::core::Persona {
        id: "bun".into(),
        name: Some("小圆".into()),
        summary: None,
        problem: None,
        avatar: None,
    }];
    assert_eq!(
        super::shown(item, &json!("bun"), Some(&installed), &texts),
        "小圆"
    );
    assert_eq!(
        super::shown(item, &json!("pudding"), Some(&installed), &texts),
        "pudding（未安装）"
    );
    assert_eq!(
        super::shown(item, &json!(null), Some(&installed), &texts),
        "内置"
    );
}

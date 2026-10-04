//! 写了几种语言的资源（蓝图 `tui.md`「界面语言」）：界面上的字一种语言一份（`text/<代码>.json`），命令的说明、运行状态行
//! 的词在同一份 JSON 里每种语言各写一份，读的时候挑界面语言那一种，读出来和只写一种的一样。

use serde::Deserialize;

use crate::language::{Language, LanguageTable};

/// 每种语言界面上的字，编译时带进来。加一种语言在这里加一行，代码和 `languages.json` 的对得上（测试查）。
const TEXTS: &[(&str, &str)] = &[
    ("zh", include_str!("../../resources/text/zh.json")),
    ("en", include_str!("../../resources/text/en.json")),
    ("ja", include_str!("../../resources/text/ja.json")),
];

/// `language` 那一份界面上的字（JSON 原文）。
///
/// # Errors
///
/// 这种语言没编进来。
pub(super) fn text(language: &Language) -> Result<(String, &'static str), String> {
    let code = language.code();
    TEXTS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(c, json)| (format!("text/{c}.json"), *json))
        .ok_or_else(|| {
            format!("resources/text/{code}.json 没编进来（config/localize.rs 的 TEXTS）")
        })
}

/// `language` 那一份里收起那一行的说法（界面语言是自动时借用，蓝图「时间线」第 17 条）。
///
/// # Errors
///
/// 这种语言没编进来，或者那一份的 `summary` 写坏了。
pub(super) fn summary(language: &Language) -> Result<super::timeline::Summary, String> {
    let (name, json) = text(language)?;
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("resources/{name} 读不懂：{e}"))?;
    serde_json::from_value(value.get("summary").cloned().unwrap_or_default())
        .map_err(|e| format!("resources/{name} 的 summary 读不懂：{e}"))
}

/// 写了几种语言的（只拿表里的语言代码做键的一格）挑 `language` 那一种，再照 `T` 读（命令的说明、运行状态行的词）。
pub(super) fn localized<T: for<'de> Deserialize<'de>>(
    name: &str,
    json: &str,
    language: &Language,
    table: &LanguageTable,
) -> Result<T, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("resources/{name} 读不懂：{e}"))?;
    pick(&mut value, language.code(), table);
    serde_json::from_value(value).map_err(|e| format!("resources/{name} 读不懂：{e}"))
}

/// 一层层找只拿语言代码做键的对象，换成 `code` 那一种（没写的退回表里退回的那种）。
fn pick(value: &mut serde_json::Value, code: &str, table: &LanguageTable) {
    match value {
        serde_json::Value::Object(map) if is_localized(map, table) => {
            let chosen = map.get(code).or_else(|| map.get(&table.fallback)).cloned();
            if let Some(chosen) = chosen {
                *value = chosen;
            }
        }
        serde_json::Value::Object(map) => map.values_mut().for_each(|v| pick(v, code, table)),
        serde_json::Value::Array(list) => list.iter_mut().for_each(|v| pick(v, code, table)),
        _ => {}
    }
}

/// 只拿表里的语言代码做键。
fn is_localized(map: &serde_json::Map<String, serde_json::Value>, table: &LanguageTable) -> bool {
    let codes = table.codes();
    !map.is_empty() && map.keys().all(|k| codes.contains(&k.as_str()))
}

#[cfg(test)]
mod tests {
    use super::{TEXTS, is_localized};
    use crate::config::Config;
    use crate::language::LanguageTable;

    fn keys(v: &serde_json::Value, at: &str, out: &mut Vec<String>) {
        if let Some(map) = v.as_object() {
            for (k, v) in map {
                let here = format!("{at}/{k}");
                out.push(here.clone());
                keys(v, &here, out);
            }
        }
    }

    fn tree(json: &str) -> Vec<String> {
        let mut out = Vec::new();
        keys(&serde_json::from_str(json).unwrap(), "", &mut out);
        out.sort();
        out
    }

    #[test]
    fn every_language_in_the_table_is_built_in_with_the_same_keys() {
        // 2026-09-30 项目主人定做英文界面，同一天加日文：每份界面文字的格一模一样。
        let table = LanguageTable::builtin().unwrap();
        let built: Vec<&str> = TEXTS.iter().map(|(c, _)| *c).collect();
        assert_eq!(built, table.codes(), "表里的和编进来的对得上");
        let zh = tree(TEXTS[0].1);
        for (code, json) in TEXTS {
            assert_eq!(tree(json), zh, "text/{code}.json 的格和中文的不一样");
        }
    }

    #[test]
    fn every_localized_entry_is_written_in_every_language() {
        // 命令的说明、运行状态行的词：每处每种语言都写了，少写的当场报（加一种语言时漏了哪处一眼看得到）。
        let table = LanguageTable::builtin().unwrap();
        fn walk(v: &serde_json::Value, at: &str, table: &LanguageTable, missing: &mut Vec<String>) {
            match v {
                serde_json::Value::Object(map) if map.contains_key("zh") => {
                    assert!(is_localized(map, table), "{at} 有不在表里的键");
                    for code in table.codes() {
                        if !map.contains_key(code) {
                            missing.push(format!("{at}/{code}"));
                        }
                    }
                }
                serde_json::Value::Object(map) => {
                    for (k, v) in map {
                        walk(v, &format!("{at}/{k}"), table, missing);
                    }
                }
                serde_json::Value::Array(list) => {
                    for (i, v) in list.iter().enumerate() {
                        walk(v, &format!("{at}/{i}"), table, missing);
                    }
                }
                _ => {}
            }
        }
        let mut missing = Vec::new();
        for (name, json) in [
            (
                "commands.json",
                include_str!("../../resources/commands.json"),
            ),
            ("pulse.json", include_str!("../../resources/pulse.json")),
        ] {
            walk(
                &serde_json::from_str(json).unwrap(),
                name,
                &table,
                &mut missing,
            );
        }
        assert!(missing.is_empty(), "没写的：{missing:?}");
    }

    #[test]
    fn each_language_picks_its_own_words() {
        let table = LanguageTable::builtin().unwrap();
        let en = Config::load(&table.find("en").unwrap(), true).unwrap();
        let undo = en
            .commands
            .commands
            .iter()
            .find(|c| c.name == "undo")
            .unwrap();
        assert!(undo.summary.starts_with("Undo"), "{}", undo.summary);
        assert!(
            en.pulse.tiers[0].words.iter().all(|w| w.is_ascii()),
            "英文的词"
        );
        assert_eq!(en.text.language_switched, "Interface language: English");
        let zh = Config::load(&table.find("zh").unwrap(), true).unwrap();
        assert!(
            zh.commands
                .commands
                .iter()
                .any(|c| c.summary.contains("撤销"))
        );
        let ja = Config::load(&table.find("ja").unwrap(), false).unwrap();
        assert_eq!(ja.text.language_switched, "表示言語：日本語");
        assert_eq!(ja.language, table.find("ja").unwrap());
        assert!(
            ja.pulse.tiers[0].words.iter().all(|w| !w.is_ascii()),
            "日文的词"
        );
    }
}

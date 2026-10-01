//! 界面语言（蓝图 `tui.md`「界面语言」，2026-09-30 项目主人定）：一张表（`resources/languages.json`），现在是中文、
//! 英文、日文。启动时照系统语言定，`/language` 开一个框选。界面上的字、命令的说明、运行状态行的词都照它换；工具的
//! 显示名照它向核心要（`human.get`，`human.rs`）。

use serde::Deserialize;

/// 界面用哪种语言：表里的代码（`zh`、`en`、`ja`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language(String);

impl Language {
    /// 资源里用的写法：`zh`、`en`、`ja`。
    pub fn code(&self) -> &str {
        &self.0
    }
}

/// 语言表：有哪几种、认不出系统语言时用哪种。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageTable {
    /// 认不出系统语言时用的，也是别的资源缺了这种语言时退回的。
    pub fallback: String,
    /// 界面语言是自动时，时间线收起那一行用哪种的说法（蓝图「时间线」第 17 条）。
    pub auto_summary: String,
    /// 有哪几种，`/language` 的框照这个顺序列。
    pub languages: Vec<Entry>,
}

/// 表里的一种。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// 代码：`text/<代码>.json`，`commands.json`、`pulse.json` 里的键。
    pub code: String,
    /// 它自己的名字（`中文`、`English`、`日本語`），框里写这个。
    pub name: String,
    /// 系统语言以这些开头的认成它（不分大小写）。
    pub locales: Vec<String>,
}

impl LanguageTable {
    /// 读编译时带进来的那一份。
    ///
    /// # Errors
    ///
    /// JSON 写坏了、退回的那种不在表里时返回错误。
    pub fn builtin() -> Result<Self, String> {
        let table: Self = serde_json::from_str(include_str!("../resources/languages.json"))
            .map_err(|e| format!("resources/languages.json 读不懂：{e}"))?;
        for (what, code) in [
            ("退回的", &table.fallback),
            ("自动时收起行用的", &table.auto_summary),
        ] {
            if table.find(code).is_none() {
                return Err(format!("resources/languages.json：{what} {code} 不在表里"));
            }
        }
        Ok(table)
    }

    /// 照系统语言：`LC_ALL`、`LC_MESSAGES`、`LANG` 依次看第一个不空的，照表里的前缀认；认不出、都没有的用退回的那种。
    pub fn detect(&self, var: impl Fn(&str) -> Option<String>) -> Language {
        let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| var(name).filter(|v| !v.trim().is_empty()))
            .map(|l| l.to_ascii_lowercase());
        let found = locale.and_then(|l| {
            self.languages.iter().find(|entry| {
                entry
                    .locales
                    .iter()
                    .any(|prefix| l.starts_with(&prefix.to_ascii_lowercase()))
            })
        });
        Language(found.map_or_else(|| self.fallback.clone(), |e| e.code.clone()))
    }

    /// 代码是 `code` 的那一种。
    pub fn find(&self, code: &str) -> Option<Language> {
        self.languages
            .iter()
            .find(|e| e.code == code)
            .map(|e| Language(e.code.clone()))
    }

    /// 代码是 `code` 的那一种自己的名字（`中文`、`English`）。
    pub fn name<'a>(&'a self, language: &'a Language) -> &'a str {
        self.languages
            .iter()
            .find(|e| e.code == language.0)
            .map_or(language.code(), |e| e.name.as_str())
    }

    /// 第 `index` 种。
    pub fn nth(&self, index: usize) -> Option<Language> {
        self.languages.get(index).map(|e| Language(e.code.clone()))
    }

    /// `language` 在表里排第几。
    pub fn position(&self, language: &Language) -> Option<usize> {
        self.languages.iter().position(|e| e.code == language.0)
    }

    /// 有哪几种的代码。
    pub fn codes(&self) -> Vec<&str> {
        self.languages.iter().map(|e| e.code.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::LanguageTable;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_string())
        }
    }

    #[test]
    fn the_system_locale_picks_the_language() {
        let table = LanguageTable::builtin().unwrap();
        let code = |pairs: &[(&str, &str)]| table.detect(env(pairs)).code().to_string();
        assert_eq!(code(&[("LANG", "zh_CN.UTF-8")]), "zh");
        assert_eq!(code(&[("LANG", "en_US.UTF-8")]), "en");
        assert_eq!(
            code(&[("LANG", "ja_JP.UTF-8")]),
            "ja",
            "2026-09-30 加的日文"
        );
        assert_eq!(
            code(&[("LC_ALL", "en_GB.UTF-8"), ("LANG", "zh_CN.UTF-8")]),
            "en",
            "LC_ALL 在前"
        );
        assert_eq!(
            code(&[("LC_ALL", ""), ("LANG", "zh_TW.UTF-8")]),
            "zh",
            "空的跳过"
        );
        assert_eq!(code(&[("LANG", "C")]), "en", "认不出的用退回的那种");
        assert_eq!(code(&[("LANG", "de_DE.UTF-8")]), "en");
        assert_eq!(code(&[]), "en", "都没有的用退回的那种");
    }

    #[test]
    fn the_table_lists_three_languages_by_their_own_names() {
        let table = LanguageTable::builtin().unwrap();
        let names: Vec<&str> = table.languages.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["中文", "English", "日本語"]);
        let ja = table.find("ja").unwrap();
        assert_eq!(table.position(&ja), Some(2));
        assert_eq!(table.nth(2), Some(ja));
        assert!(table.find("fr").is_none());
    }
}

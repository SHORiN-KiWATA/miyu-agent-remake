//! 给人看的一句话的几种语言：人格的名字、说明（施工 P-1 上），软件包清单的名字、说明、子命令的说明（施工 9-1 上）。写法是
//! 一张表，语言代码到那一句：`{ en = "Web", zh = "网页" }`。只认 `zh`、`en`、`ja`，每一句去掉前后空白不能是空的。
//!
//! 人格、预设的名字、说明另有 [`Label`]（施工 P-3 补，2026-10-08 项目主人定：名字只存一份、不分语言）：写成一句字，
//! 以前写成语言表的照样读。

use std::collections::BTreeMap;
use std::ops::Range;

use toml_edit::{Item, TableLike};

/// 认的语言。
pub const LANGUAGES: [&str; 3] = ["zh", "en", "ja"];

/// 一句话的几种语言：语言代码到那一句。
pub type Phrases = BTreeMap<String, String>;

/// 人格、预设的名字、说明：一句字；以前写成语言表的（出厂的几个照旧这么写，好照连接的语言显示）照样认。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Label {
    /// 一句字。
    One(String),
    /// 语言到一句话。
    Each(Phrases),
}

impl Label {
    /// 照语言 `language` 挑一句：一句字的就是它，空的字（没有说明，施工 P-3 再补）没有；语言表的照 `language`、`en`、
    /// `zh`、`ja` 的先后挑，都没写的没有。
    pub fn pick(&self, language: &str) -> Option<&str> {
        match self {
            Label::One(text) => Some(text.as_str()).filter(|text| !text.is_empty()),
            Label::Each(phrases) => [language, "en", "zh", "ja"]
                .iter()
                .find_map(|language| phrases.get(*language))
                .map(String::as_str),
        }
    }
}

/// 读一格名字、说明：是字的去掉前后空白、不能是空的；是表的照 [`read`]。
///
/// # Errors
///
/// 是空的字；不是字也不是表；表里有不认识的语言、空的一句。
pub fn read_label(item: &Item) -> Result<Label, PhraseError> {
    match item.as_str().map(str::trim) {
        Some(text) if !text.is_empty() => Ok(Label::One(text.to_string())),
        Some(_) => Err(PhraseError::Empty(String::new(), item.span())),
        None => read(item).map(Label::Each),
    }
}

/// 读一格说明（施工 P-3 再补，2026-10-08 项目主人：「为什么说明不让为空？」）：空的字（去掉前后空白是空的）就是没有说明，
/// 盖住下面那一层的；别的同 [`read_label`]。
///
/// # Errors
///
/// 同 [`read_label`]，空的字除外。
pub fn read_summary(item: &Item) -> Result<Label, PhraseError> {
    match item.as_str().map(str::trim) {
        Some("") => Ok(Label::One(String::new())),
        _ => read_label(item),
    }
}

/// 读不成的一处，带着它在原文里的位置（字节范围，原文没有位置的是没有）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhraseError {
    /// 不是一张表。
    NotPhrases(Option<Range<usize>>),
    /// 不认识的语言：哪一种。
    UnknownLanguage(String, Option<Range<usize>>),
    /// 这种语言那一句不是字、或者是空的。
    Empty(String, Option<Range<usize>>),
}

/// 读一格「语言到一句话」：去掉前后空白存。
///
/// # Errors
///
/// 不是一张表、有不认识的语言、有一句是空的或者不是字，交回第一处。
pub fn read(item: &Item) -> Result<Phrases, PhraseError> {
    let Some(table) = item.as_table_like() else {
        return Err(PhraseError::NotPhrases(item.span()));
    };
    let mut phrases = Phrases::new();
    for (language, value) in TableLike::iter(table) {
        if !LANGUAGES.contains(&language) {
            return Err(PhraseError::UnknownLanguage(
                language.to_string(),
                value.span(),
            ));
        }
        match value.as_str().map(str::trim) {
            Some(text) if !text.is_empty() => {
                phrases.insert(language.to_string(), text.to_string());
            }
            _ => return Err(PhraseError::Empty(language.to_string(), value.span())),
        }
    }
    Ok(phrases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_is_one_line_or_a_table_and_picks_by_language() {
        let one = read_label(&item("x = \"  我的  \"")).unwrap();
        assert_eq!(one, Label::One("我的".to_string()));
        assert_eq!(one.pick("ja"), Some("我的"), "一句字的不分语言");
        let each = read_label(&item("x = { zh = \"开发\", en = \"Dev\" }")).unwrap();
        assert_eq!(
            (each.pick("zh"), each.pick("ja")),
            (Some("开发"), Some("Dev")),
            "没有的语言照 en、zh、ja 的先后"
        );
        assert!(matches!(
            read_label(&item("x = \" \"")),
            Err(PhraseError::Empty(..))
        ));
        assert!(matches!(
            read_label(&item("x = 3")),
            Err(PhraseError::NotPhrases(_))
        ));
    }

    /// 说明可以是空的字：就是没有说明，挑不出一句（施工 P-3 再补）；语言表里的一句照旧不能空，名字照旧不收空的。
    #[test]
    fn a_summary_may_be_empty_and_then_picks_nothing() {
        let empty = read_summary(&item("x = \"  \"")).unwrap();
        assert_eq!(empty, Label::One(String::new()));
        assert_eq!(empty.pick("zh"), None);
        assert_eq!(
            read_summary(&item("x = \" 写代码 \"")).unwrap(),
            Label::One("写代码".to_string())
        );
        assert!(read_summary(&item("x = { en = \" \" }")).is_err());
        assert!(read_label(&item("x = \"\"")).is_err());
    }

    fn item(text: &str) -> Item {
        let document: toml_edit::Document<String> = text.parse().unwrap();
        document.as_table().get("x").unwrap().clone()
    }

    #[test]
    fn languages_map_to_trimmed_text() {
        let phrases = read(&item("x = { en = \" Web \", zh = \"网页\" }\n")).unwrap();
        assert_eq!(phrases.get("en").map(String::as_str), Some("Web"));
        assert_eq!(phrases.len(), 2);
    }

    #[test]
    fn the_first_wrong_one_is_reported() {
        assert!(matches!(
            read(&item("x = 1\n")),
            Err(PhraseError::NotPhrases(_))
        ));
        assert!(matches!(
            read(&item("x = { fr = \"a\" }\n")),
            Err(PhraseError::UnknownLanguage(language, _)) if language == "fr"
        ));
        assert!(matches!(
            read(&item("x = { en = \" \" }\n")),
            Err(PhraseError::Empty(language, _)) if language == "en"
        ));
        assert!(matches!(
            read(&item("x = { en = 1 }\n")),
            Err(PhraseError::Empty(language, _)) if language == "en"
        ));
    }
}

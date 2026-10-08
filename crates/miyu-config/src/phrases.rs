//! 给人看的一句话的几种语言：人格的名字、说明（施工 P-1 上），软件包清单的名字、说明、子命令的说明（施工 9-1 上）。写法是
//! 一张表，语言代码到那一句：`{ en = "Web", zh = "网页" }`。只认 `zh`、`en`、`ja`，每一句去掉前后空白不能是空的。

use std::collections::BTreeMap;
use std::ops::Range;

use toml_edit::{Item, TableLike};

/// 认的语言。
pub const LANGUAGES: [&str; 3] = ["zh", "en", "ja"];

/// 一句话的几种语言：语言代码到那一句。
pub type Phrases = BTreeMap<String, String>;

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

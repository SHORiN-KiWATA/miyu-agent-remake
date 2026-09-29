//! `miyu redo` 的参数（施工 4-7 再补）：写的几个词用一个空格连起来；一个都没写的没有 `text` 这一格，原样重做。

use super::*;
use crate::language::Language;

/// 照 `words` 重做上一次 `miyu ask` 开的会话。
fn redoing(words: &[&str]) -> RedoPlan {
    let args = Redo {
        words: words.iter().map(ToString::to_string).collect(),
        session: None,
    };
    RedoPlan {
        text: args.text(),
        session: args.session,
        language: Language::Chinese,
        human: Human::default(),
        home: None,
    }
}

#[test]
fn words_are_joined_and_none_means_as_it_was() {
    assert_eq!(
        request("s1", &redoing(&["换个", "说法"])),
        json!({"session": "s1", "text": "换个 说法"})
    );
    assert_eq!(request("s1", &redoing(&[])), json!({"session": "s1"}));
}

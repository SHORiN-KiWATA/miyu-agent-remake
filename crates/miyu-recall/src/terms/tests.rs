//! 切词（`recall.md` 第一条）。

use super::*;

#[test]
fn chinese_is_cut_into_overlapping_pairs_and_single_characters() {
    assert_eq!(index_terms("记忆系统"), "记 记忆 忆 忆系 系 系统 统");
}

#[test]
fn a_lone_character_is_its_own_term() {
    assert_eq!(index_terms("猫"), "猫");
    assert_eq!(index_terms("一只 猫"), "一 一只 只 猫");
}

#[test]
fn kana_and_kanji_run_together_and_apart_from_latin_and_digits() {
    // 平假名、片假名、汉字连成一段；英文、数字、标点照原样交给 unicode61。
    assert_eq!(
        index_terms("東京タワー"),
        "東 東京 京 京タ タ タワ ワ ワー ー"
    );
    assert_eq!(index_terms("用Rust写"), "用 Rust 写");
    assert_eq!(index_terms("N卡3090显卡"), "N 卡 3090 显 显卡 卡");
}

#[test]
fn the_katakana_middle_dot_and_punctuation_break_a_run() {
    assert_eq!(index_terms("ア・イ"), "ア ・ イ");
    assert_eq!(index_terms("你好，世界"), "你 你好 好 ， 世 世界 界");
}

#[test]
fn a_query_uses_only_pairs_of_a_long_run_and_the_lone_character() {
    assert_eq!(
        query("记忆系统").as_deref(),
        Some(r#""记忆" OR "忆系" OR "系统""#)
    );
    assert_eq!(query("猫").as_deref(), Some(r#""猫""#));
}

#[test]
fn a_query_splits_latin_words_and_lowercases_them() {
    assert_eq!(
        query("Rust 的 FTS5, rust!").as_deref(),
        Some(r#""rust" OR "的" OR "fts5""#)
    );
}

#[test]
fn a_query_keeps_the_first_of_each_term_and_at_most_the_limit() {
    assert_eq!(query("猫 猫 狗").as_deref(), Some(r#""猫" OR "狗""#));
    let long: String = (0..200).map(|n| format!("w{n} ")).collect();
    let terms = query(&long).unwrap();
    assert_eq!(terms.matches(" OR ").count(), MAX_QUERY_TERMS - 1);
    assert!(terms.starts_with(r#""w0" OR "w1""#));
}

#[test]
fn keywords_of_fts5_are_quoted_like_any_word() {
    assert_eq!(
        query("NOT or NEAR").as_deref(),
        Some(r#""not" OR "or" OR "near""#)
    );
}

#[test]
fn nothing_to_search_is_none() {
    assert_eq!(query(""), None);
    assert_eq!(query("  ，。！ ...\n"), None);
}

//! 分词（`recall.md` 第四条第 5 款）：真词表上照 Hugging Face `tokenizers` 0.23.2 的结果逐个比（`fixtures/tokens.json`，
//! `fixtures/make.py` 生成；词表是 bge-small-zh-v1.5 的，FlagEmbedding 的 MIT 许可证在 `fixtures/bge-vocab.LICENSE`）。
//! 几样边界另外测：截断留住 `[SEP]`、字里写着 `[CLS]` 照普通的字切、词表坏了说清楚。

use miyu_embed::wordpiece::{Tokenizer, VocabError};
use serde::Deserialize;

use crate::support::read;

#[derive(Deserialize)]
struct Case {
    text: String,
    ids: Vec<i64>,
}

fn bge() -> Tokenizer {
    Tokenizer::new(&read("bge-vocab.txt")).expect("读得出词表")
}

#[test]
fn every_sentence_matches_the_reference() {
    let tokenizer = bge();
    let cases: Vec<Case> = serde_json::from_str(&read("tokens.json")).expect("合写法");
    assert!(cases.len() > 30, "{}", cases.len());
    for case in cases {
        assert_eq!(
            tokenizer.encode(&case.text, 512),
            case.ids,
            "{:?}",
            case.text
        );
    }
}

#[test]
fn a_long_text_is_cut_and_keeps_the_sep() {
    let ids = bge().encode(&"猫".repeat(20), 6);
    assert_eq!(ids, [101, 4344, 4344, 4344, 4344, 102]);
    assert_eq!(bge().encode("猫", 6), [101, 4344, 102], "短的不动");
    assert_eq!(
        bge().encode(&"猫".repeat(4), 6),
        [101, 4344, 4344, 4344, 4344, 102],
        "正好放下"
    );
}

#[test]
fn a_special_word_in_the_text_is_plain_text() {
    let ids = bge().encode("[CLS]", 512);
    assert_eq!(ids.first(), Some(&101));
    assert_eq!(ids.last(), Some(&102));
    assert_eq!(
        ids.iter().filter(|id| **id == 101).count(),
        1,
        "字里的不算：{ids:?}"
    );
}

#[test]
fn a_vocab_without_the_special_words_is_refused() {
    let error = Tokenizer::new("猫\n狗\n").expect_err("没有那几个词");
    assert_eq!(error, VocabError::Missing("[CLS]".to_string()));
    assert_eq!(error.to_string(), "vocab has no [CLS]");
    let error = Tokenizer::new("[CLS]\n[SEP]\n").expect_err("没有 [UNK]");
    assert_eq!(error, VocabError::Missing("[UNK]".to_string()));
    let error = Tokenizer::new("[CLS]\n[SEP]\n[UNK]\n猫\n猫\n").expect_err("重了");
    assert_eq!(error, VocabError::Repeated("猫".to_string()));
    assert_eq!(error.to_string(), "vocab has 猫 twice");
}

#[test]
fn ids_are_the_line_numbers() {
    let tokenizer = Tokenizer::new("[PAD]\n[UNK]\n[CLS]\n[SEP]\n猫\n").expect("读得出");
    assert_eq!(tokenizer.encode("猫狗", 8), [2, 4, 1, 3]);
    let crlf =
        Tokenizer::new("[PAD]\r\n[UNK]\r\n[CLS]\r\n[SEP]\r\n猫\r\n").expect("Windows 的换行也认");
    assert_eq!(crlf.encode("猫", 8), [2, 4, 3]);
}

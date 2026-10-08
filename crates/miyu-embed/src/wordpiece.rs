//! BERT 的 WordPiece 分词（施工 R-5 上，`docs/blueprint/recall.md` 第四条第 5 款）：照 bge-small-zh-v1.5 原版的配置，和
//! Hugging Face `tokenizers` 的 `BertNormalizer`（不转小写、不去重音）、`BertPreTokenizer`、`WordPiece` 切得一样
//! （`tests/tokens.rs` 照它的结果逐个比）。不用 `tokenizers` 这个库：只要这一种，它依赖多、程序大。
//!
//! 和它不一样的只有两处：
//! - 字里写着 `[CLS]` 这种的，它当成那个特殊的词，这里照普通的字切：人说的话不变成控制用的词。
//! - Unicode 的类别照 `unicode-general-category` 的表（新），它的表是 Unicode 9 的：9 以后才有的字（新的 emoji）它当成
//!   没分配、去掉，这里切成 `[UNK]`。

use std::collections::BTreeMap;
use std::fmt;

use unicode_general_category::{GeneralCategory, get_general_category};

/// 一个词超过这么多个字整个算 `[UNK]`（`tokenizers` 的 `max_input_chars_per_word`，BERT 的默认）。
const LONGEST_WORD: usize = 100;

/// 接在词中间那一段前面的记号。
const CONTINUING: &str = "##";

/// 一份词表照 WordPiece 切。
#[derive(Debug, Clone)]
pub struct Tokenizer {
    /// 词和它的编号（行号）。
    vocab: BTreeMap<String, i64>,
    cls: i64,
    sep: i64,
    unk: i64,
}

/// 词表读不出来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VocabError {
    /// 少了 `[CLS]`、`[SEP]`、`[UNK]` 里的一个。
    Missing(String),
    /// 同一个词写了两行：编号该是哪一行说不清。
    Repeated(String),
}

impl fmt::Display for VocabError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VocabError::Missing(word) => write!(f, "vocab has no {word}"),
            VocabError::Repeated(word) => write!(f, "vocab has {word} twice"),
        }
    }
}

impl std::error::Error for VocabError {}

impl Tokenizer {
    /// 照词表的原文 `vocab` 造：一行一个词，第几行（从 0 数）就是编号；`\n`、`\r\n` 都认。
    ///
    /// # Errors
    ///
    /// 没有 `[CLS]`、`[SEP]`、`[UNK]`，或者一个词写了两行。
    pub fn new(vocab: &str) -> Result<Tokenizer, VocabError> {
        let mut words = BTreeMap::new();
        for (id, word) in (0_i64..).zip(vocab.lines()) {
            if words.insert(word.to_string(), id).is_some() {
                return Err(VocabError::Repeated(word.to_string()));
            }
        }
        let special = |word: &str| {
            words
                .get(word)
                .copied()
                .ok_or_else(|| VocabError::Missing(word.to_string()))
        };
        Ok(Tokenizer {
            cls: special("[CLS]")?,
            sep: special("[SEP]")?,
            unk: special("[UNK]")?,
            vocab: words,
        })
    }

    /// 把 `text` 切成编号：`[CLS]`、一个个词、`[SEP]`。超过 `limit` 个的截掉后面的，留住 `[SEP]`；`limit` 至少是 2
    /// （清单查过，`manifest.rs`），更小的照 2 算。
    pub fn encode(&self, text: &str, limit: usize) -> Vec<i64> {
        let mut ids = vec![self.cls];
        for word in words(text) {
            self.pieces(&word, &mut ids);
        }
        ids.truncate(limit.max(2) - 1);
        ids.push(self.sep);
        ids
    }

    /// 一个词照 WordPiece 切：从头起每次取最长的、在词表里的一段（不是开头的带 `##`），有一段取不出来整个词算 `[UNK]`。
    fn pieces(&self, word: &str, ids: &mut Vec<i64>) {
        if word.chars().count() > LONGEST_WORD {
            ids.push(self.unk);
            return;
        }
        let mut found = Vec::new();
        let mut start = 0;
        while start < word.len() {
            let piece = word[start..]
                .char_indices()
                .map(|(at, c)| start + at + c.len_utf8())
                .rev()
                .find_map(|end| {
                    let piece = &word[start..end];
                    let key = if start == 0 {
                        piece.to_string()
                    } else {
                        format!("{CONTINUING}{piece}")
                    };
                    self.vocab.get(&key).map(|id| (*id, end))
                });
            let Some((id, end)) = piece else {
                ids.push(self.unk);
                return;
            };
            found.push(id);
            start = end;
        }
        ids.extend(found);
    }
}

/// 照 `BertNormalizer`、`BertPreTokenizer` 把 `text` 切成词：去掉控制字符和 `U+FFFD`、`U+0000`，空白处断开，汉字、标点
/// 一个一个成词。
fn words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c == '\0' || c == '\u{fffd}' || control(c) {
            continue;
        }
        if c.is_whitespace() || chinese(c) || punctuation(c) {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            if !c.is_whitespace() {
                words.push(c.to_string());
            }
            continue;
        }
        word.push(c);
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

/// 要去掉的：Unicode 的「其他」类（控制、格式、代理、私用、没分配），制表、换行、回车除外（它们算空白）。
fn control(c: char) -> bool {
    if matches!(c, '\t' | '\n' | '\r') {
        return false;
    }
    matches!(
        get_general_category(c),
        GeneralCategory::Control
            | GeneralCategory::Format
            | GeneralCategory::Surrogate
            | GeneralCategory::PrivateUse
            | GeneralCategory::Unassigned
    )
}

/// 标点：ASCII 的标点（含 `$`、`+`、`^` 这些符号），和 Unicode 的七种标点类。
fn punctuation(c: char) -> bool {
    c.is_ascii_punctuation()
        || matches!(
            get_general_category(c),
            GeneralCategory::ConnectorPunctuation
                | GeneralCategory::DashPunctuation
                | GeneralCategory::ClosePunctuation
                | GeneralCategory::FinalPunctuation
                | GeneralCategory::InitialPunctuation
                | GeneralCategory::OtherPunctuation
                | GeneralCategory::OpenPunctuation
        )
}

/// 汉字：`tokenizers` 认的那几段 CJK 统一表意文字（基本区、扩展 A 到 F 里的几段、兼容区和它的补充）。
fn chinese(c: char) -> bool {
    matches!(
        u32::from(c),
        0x4E00..=0x9FFF
            | 0x3400..=0x4DBF
            | 0x20000..=0x2A6DF
            | 0x2A700..=0x2B73F
            | 0x2B740..=0x2B81F
            | 0x2B920..=0x2CEAF
            | 0xF900..=0xFAFF
            | 0x2F800..=0x2FA1F
    )
}

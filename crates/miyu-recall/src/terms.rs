//! 切词（`docs/blueprint/recall.md`「怎么走」第一条，施工 R-1）。
//!
//! 一段字先分成一段段：连着的汉字、平假名、片假名是一段（[`is_cjk`]），别的是另一种段。汉字假名那种两两重叠地切，每个字
//! 也单独是一个词；别的照原样交给 FTS5 的 `unicode61`（它照空白、标点切，大小写不分）。为什么不用词典（jieba）：中日文都行、
//! 三个平台一样、没有词典要带要更新，新词人名也切得出（`docs/reviews/2026-10-07-记忆知识库embedding调研.md` 第三节）。

#[cfg(test)]
mod tests;

/// 一句查询最多出几个词：一句话再长，查询也有个上限（`recall.md` 第一条第 5 款）。
pub const MAX_QUERY_TERMS: usize = 64;

/// 一段字切成存进 FTS5 那一列的词，词和词之间一个空格。
///
/// 汉字假名的段：每个字一个词，挨着的两个字再一个词（`记忆系统` → `记 记忆 忆 忆系 系 系统 统`）。单字也存，是为了只问
/// 一个字的（「猫」）也搜得到；常见的单字 bm25 的分低，不会喧宾夺主。别的段去掉前后空白原样放，交给 `unicode61` 去切。
pub fn index_terms(text: &str) -> String {
    let mut terms: Vec<String> = Vec::new();
    for segment in segments(text) {
        match segment {
            Segment::Cjk(chars) => {
                for (i, c) in chars.iter().enumerate() {
                    terms.push(c.to_string());
                    if let Some(next) = chars.get(i + 1) {
                        terms.push([*c, *next].iter().collect());
                    }
                }
            }
            Segment::Other(other) => {
                let other = other.trim();
                if !other.is_empty() {
                    terms.push(other.to_string());
                }
            }
        }
    }
    terms.join(" ")
}

/// 一句话拼成 FTS5 的 `MATCH` 写法；一个词都切不出来的交回 `None`，不查。
///
/// 汉字假名的段，两个字以上的只出挨着的两两（长段里的单字只是噪声），只有一个字的出那个字；别的段照 `unicode61` 的规矩
/// 切成词（字母、数字连着的算一个），转成小写。去重、照先后留前 [`MAX_QUERY_TERMS`] 个，每个包上双引号、用 ` OR ` 连：中了
/// 越多、越少见的 bm25 越靠前。双引号也让 `OR`、`NOT`、`NEAR` 这些 FTS5 的关键字只当普通的词。词里只有字母、数字、汉字假名，
/// 不会有双引号，不用转义。
pub fn query(text: &str) -> Option<String> {
    let mut terms: Vec<String> = Vec::new();
    let mut add = |term: String| {
        if terms.len() < MAX_QUERY_TERMS && !terms.contains(&term) {
            terms.push(term);
        }
    };
    for segment in segments(text) {
        match segment {
            Segment::Cjk(chars) if chars.len() == 1 => add(chars[0].to_string()),
            Segment::Cjk(chars) => {
                for pair in chars.windows(2) {
                    add(pair.iter().collect());
                }
            }
            Segment::Other(other) => {
                for word in other.split(|c: char| !c.is_alphanumeric()) {
                    if !word.is_empty() {
                        add(word.to_lowercase());
                    }
                }
            }
        }
    }
    if terms.is_empty() {
        return None;
    }
    let quoted: Vec<String> = terms.iter().map(|term| format!("\"{term}\"")).collect();
    Some(quoted.join(" OR "))
}

/// 切开的一段。
enum Segment<'a> {
    /// 连着的汉字、假名。
    Cjk(Vec<char>),
    /// 别的：英文、数字、空白、标点，原样。
    Other(&'a str),
}

/// 照 [`is_cjk`] 把字分成一段段，照原来的先后。
fn segments(text: &str) -> Vec<Segment<'_>> {
    let mut found = Vec::new();
    let mut cjk: Vec<char> = Vec::new();
    let mut other_start: Option<usize> = None;
    for (at, c) in text.char_indices() {
        if is_cjk(c) {
            if let Some(start) = other_start.take() {
                found.push(Segment::Other(&text[start..at]));
            }
            cjk.push(c);
        } else {
            if !cjk.is_empty() {
                found.push(Segment::Cjk(std::mem::take(&mut cjk)));
            }
            other_start.get_or_insert(at);
        }
    }
    if let Some(start) = other_start {
        found.push(Segment::Other(&text[start..]));
    }
    if !cjk.is_empty() {
        found.push(Segment::Cjk(cjk));
    }
    found
}

/// 算不算汉字、假名：照 Unicode 的区块（施工单 R-1「定了的」第 3 条）。片假名的中点 `・` 是标点，不算；韩文有空格，
/// `unicode61` 切得开，也不算。
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{3005}' | '\u{3007}'          // 々 〇
        | '\u{3040}'..='\u{309F}'        // 平假名
        | '\u{30A0}'..='\u{30FA}'        // 片假名（到 ヺ）
        | '\u{30FC}'..='\u{30FF}'        // ー ヽ ヾ ヿ
        | '\u{31F0}'..='\u{31FF}'        // 片假名扩展
        | '\u{3400}'..='\u{4DBF}'        // 汉字扩展 A
        | '\u{4E00}'..='\u{9FFF}'        // 汉字
        | '\u{F900}'..='\u{FAFF}'        // 兼容汉字
        | '\u{FF66}'..='\u{FF9F}'        // 半角片假名
        | '\u{20000}'..='\u{3134F}'      // 汉字扩展 B 到 H
    )
}

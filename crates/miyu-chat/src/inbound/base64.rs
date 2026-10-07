//! 正文里的 base64（`docs/blueprint/chat.md` 第二条「怎么走」第 5、10 条）：从正文里找出一段段 base64，解成字节，筛出读得
//! 懂的字（[`Base64::reveal`]）。违规关键词查它，桥拿它给判官看（施工时定的第 15、19 条）。
//!
//! 自己写，不加依赖（O-5 施工单「风险」第 2 条）：只要标准字母表的解码，不校验填充、不报错，宽松地能解多少解多少。正文里
//! 找到的「base64」大多只是普通的英文、数字，解出来是乱码：读不成 UTF-8 的整段不要（施工时定的第 20 条），读得成的再交给
//! 可打印的比例去筛。

use std::collections::BTreeSet;

/// 正文里的 base64 的三个数：怎么解、解出来的怎么筛。出厂的数随桥那一步放进出厂的数据，代码里不写死。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Base64 {
    /// 一段至少多少个字符才去解，连末尾的 `=` 一起数。太短的大多是普通的词。
    pub min_chars: usize,
    /// 解出来最多看前多少个字符：可打印的比例、关键词、给判官看的都只有这些。
    pub max_chars: usize,
    /// 可打印的字符（不是控制字符的）至少占几成，千分比：低于它的当乱码，不要。
    pub printable: u16,
}

impl Base64 {
    /// 正文 `text` 里的 base64 解出来的字（「怎么走」第 10 条）：每一段够 `min_chars` 个字符的，解出来的字节照 UTF-8 读，
    /// 读不成的整段不要；读得成的只留前 `max_chars` 个字符，可打印的够 `printable` 的留下；留下的去掉重复的（比解出来的字），照出现的先后用换行接起来。
    /// 一段都没有是 `None`。
    ///
    /// 违规关键词查它；桥拿它填判官的 [`Ask::decoded`](crate::Ask::decoded)：解得出来就给判官看，不只是命中关键词的时候
    /// （旧版 `decode_base64_text`，施工时定的第 15 条）。
    pub fn reveal(&self, text: &str) -> Option<String> {
        let mut seen = BTreeSet::new();
        let shown: Vec<String> = segments(text)
            .into_iter()
            .filter(|segment| segment.len() >= self.min_chars)
            .filter_map(|segment| self.shown(segment))
            .filter(|shown| seen.insert(shown.clone()))
            .collect();
        (!shown.is_empty()).then(|| shown.join("\n"))
    }

    /// 一段 base64 解出来的字：字节照 UTF-8 读，只留前 `max_chars` 个字符；读不成 UTF-8 的、可打印的不够 `printable` 的、
    /// 空的是 `None`。
    ///
    /// 读不成的整段不要，不换成替换字符：替换字符不是控制字符，算可打印，长数字串、哈希这类就会解出一串乱码交给判官，
    /// 白花判官的 token（施工时定的第 20 条，照旧版 `decode_base64_text`）。
    fn shown(&self, segment: &str) -> Option<String> {
        let decoded = String::from_utf8(decode(segment)).ok()?;
        let shown: String = decoded.chars().take(self.max_chars).collect();
        let total = shown.chars().count();
        let printable = shown.chars().filter(|c| !c.is_control()).count();
        let enough =
            printable.saturating_mul(1000) >= total.saturating_mul(usize::from(self.printable));
        (total > 0 && enough).then_some(shown)
    }
}

/// 一个 base64 字符代表的六位数；不是字母表里的（包括 `=`）是 `None`。
fn sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// 正文里的每一段 base64：连续的字母表字符（`A–Z`、`a–z`、`0–9`、`+`、`/`），带上紧跟着的 `=`。别的字（空白、标点、
/// 中文……）把它截开；`=` 只在一段的末尾，后面再接字母表字符是另一段。
///
/// 段的边界都在 ASCII 字符上，切出来的一定是完整的字。
fn segments(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        while bytes.get(at).copied().and_then(sextet).is_some() {
            at += 1;
        }
        if at == start {
            at += 1;
            continue;
        }
        while bytes.get(at) == Some(&b'=') {
            at += 1;
        }
        found.extend(text.get(start..at));
    }
    found
}

/// 解一段 base64 成字节：读到第一个不是字母表的字符（`=`）为止，每四个字符三个字节；末尾剩两个、三个字符的照样解出一个、
/// 两个字节，剩一个的凑不出一个字节，丢掉。
fn decode(segment: &str) -> Vec<u8> {
    let sextets: Vec<u8> = segment.bytes().map_while(sextet).collect();
    let mut bytes = Vec::with_capacity(sextets.len() / 4 * 3 + 2);
    for chunk in sextets.chunks(4) {
        // 不满四个的，右边补零凑成 24 位。
        let word = chunk
            .iter()
            .fold(0_u32, |word, &sextet| word << 6 | u32::from(sextet))
            << (6 * (4 - chunk.len()));
        let [_, first, second, third] = word.to_be_bytes();
        let decoded = [first, second, third];
        bytes.extend_from_slice(decoded.get(..chunk.len() - 1).unwrap_or_default());
    }
    bytes
}

#[cfg(test)]
mod tests;

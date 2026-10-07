//! 违规关键词用的 base64（`docs/blueprint/chat.md` 第二条「怎么走」第 5 条）：从正文里找出一段段 base64，解成字节。
//!
//! 自己写，不加依赖（施工单「风险」第 2 条）：只要标准字母表的解码，不校验填充、不报错，宽松地能解多少解多少。正文里找到的
//! 「base64」大多只是普通的英文、数字，解出来是乱码，交给可打印的比例去筛。

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
pub(super) fn segments(text: &str) -> Vec<&str> {
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
pub(super) fn decode(segment: &str) -> Vec<u8> {
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

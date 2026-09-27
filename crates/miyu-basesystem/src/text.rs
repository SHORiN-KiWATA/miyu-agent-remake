//! 一份文本文件的写法（`10-自带软件.md` 第六节「保留文件原来的换行风格、编码和 BOM」，施工 4-6 上）：从原来的
//! 字节认出编码、BOM、换行，把她给的字照同样的写法写回字节。`write` 覆盖已有的文件时整份照它写；`edit`（施工
//! 4-6 中）严格地解开、只照它换 `new_string` 的换行，写回时只照编码、BOM。
//!
//! 认得出的编码和 `read` 一样：UTF-8（带不带 BOM），带 BOM 的 UTF-16。认不出的（没有 BOM 的别的编码）当 UTF-8。

/// 编码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
    /// UTF-8。
    Utf8,
    /// 小端的 UTF-16，带 BOM。
    Utf16Le,
    /// 大端的 UTF-16，带 BOM。
    Utf16Be,
}

/// 一份文件的写法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Style {
    /// 编码。
    pub(crate) encoding: Encoding,
    /// UTF-8 的开头有没有 BOM。UTF-16 一律带。
    pub(crate) bom: bool,
    /// 换行是 CRLF：照第一处换行认。
    pub(crate) crlf: bool,
}

/// UTF-8 的 BOM。
const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];

impl Style {
    /// 新文件的写法：UTF-8、不带 BOM、换行照给的写。
    pub(crate) fn fresh() -> Style {
        Style {
            encoding: Encoding::Utf8,
            bom: false,
            crlf: false,
        }
    }

    /// 照文件原来的字节认。
    pub(crate) fn of(bytes: &[u8]) -> Style {
        let (encoding, bom) = match bytes {
            [0xFF, 0xFE, ..] => (Encoding::Utf16Le, true),
            [0xFE, 0xFF, ..] => (Encoding::Utf16Be, true),
            _ => (Encoding::Utf8, bytes.starts_with(&UTF8_BOM)),
        };
        let text = decode(encoding, bytes);
        let crlf = text.find('\n').is_some_and(|at| text[..at].ends_with('\r'));
        Style {
            encoding,
            bom,
            crlf,
        }
    }

    /// 把 `text` 照这种写法写成字节：换行照 [`Style::lines`] 换，再照编码、BOM 写。
    pub(crate) fn encode(&self, text: &str) -> Vec<u8> {
        self.bytes(&self.lines(text))
    }

    /// 换行照这种写法写：CRLF 的，单个换行换成 CRLF（原来就是 CRLF 的不重复换）；不是的，照给的写。`edit` 只拿
    /// 它换 `new_string`，文件别的地方原样。
    pub(crate) fn lines(&self, text: &str) -> String {
        match self.crlf {
            true => text.replace("\r\n", "\n").replace('\n', "\r\n"),
            false => text.to_string(),
        }
    }

    /// 只照编码、BOM 写成字节，换行一个不动。
    pub(crate) fn bytes(&self, text: &str) -> Vec<u8> {
        match self.encoding {
            Encoding::Utf8 => {
                let mut bytes = Vec::with_capacity(text.len() + 3);
                if self.bom {
                    bytes.extend_from_slice(&UTF8_BOM);
                }
                bytes.extend_from_slice(text.as_bytes());
                bytes
            }
            Encoding::Utf16Le => utf16(text, [0xFF, 0xFE], u16::to_le_bytes),
            Encoding::Utf16Be => utf16(text, [0xFE, 0xFF], u16::to_be_bytes),
        }
    }

    /// 严格地解成字，BOM 不算在字里：UTF-8 的要合写法，UTF-16 的要是整数个两字节、没有落单的代理项。解不开的
    /// 是空的：`edit` 改完要写回去，解不开的字节写回去就坏了。
    pub(crate) fn strict(&self, bytes: &[u8]) -> Option<String> {
        match self.encoding {
            Encoding::Utf8 => {
                let body = bytes.strip_prefix(&UTF8_BOM).unwrap_or(bytes);
                std::str::from_utf8(body).ok().map(str::to_string)
            }
            Encoding::Utf16Le => strict_units(bytes, u16::from_le_bytes),
            Encoding::Utf16Be => strict_units(bytes, u16::from_be_bytes),
        }
    }
}

/// UTF-16 的字节严格地解成字：跳过 BOM，要整数个两字节、没有落单的代理项。
fn strict_units(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> Option<String> {
    let body = bytes.get(2..)?;
    if body.len() % 2 != 0 {
        return None;
    }
    let units: Vec<u16> = body
        .chunks_exact(2)
        .map(|pair| unit([pair[0], pair[1]]))
        .collect();
    String::from_utf16(&units).ok()
}

/// 照编码解成字，BOM 不算在字里；解不了的字节换成 `�`。
pub(crate) fn decode(encoding: Encoding, bytes: &[u8]) -> String {
    match encoding {
        Encoding::Utf8 => {
            let body = bytes.strip_prefix(&UTF8_BOM).unwrap_or(bytes);
            String::from_utf8_lossy(body).into_owned()
        }
        Encoding::Utf16Le => units(bytes, u16::from_le_bytes),
        Encoding::Utf16Be => units(bytes, u16::from_be_bytes),
    }
}

/// UTF-16 的字节解成字：跳过两个字节的 BOM。
fn units(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> String {
    let body = bytes.get(2..).unwrap_or_default();
    let units: Vec<u16> = body
        .chunks_exact(2)
        .map(|pair| unit([pair[0], pair[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

/// 字写成带 BOM 的 UTF-16。
fn utf16(text: &str, bom: [u8; 2], bytes: fn(u16) -> [u8; 2]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 2 + 2);
    out.extend_from_slice(&bom);
    for unit in text.encode_utf16() {
        out.extend_from_slice(&bytes(unit));
    }
    out
}

/// 一段字有几行：以换行结尾的，最后那一段空的不算；什么都没有的是 0 行。给人看的说法里用。
pub(crate) fn line_count(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    text.matches('\n').count() + usize::from(!text.ends_with('\n'))
}

#[cfg(test)]
mod tests;

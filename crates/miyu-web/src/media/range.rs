//! 分段、下载的名字（`web-ui.md`「怎么走」第三条第 6、7 款）。

/// `Range` 要的是哪一段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Span {
    /// 全部：没写、写了好几段、写法不对的（照没写，`web-ui.md`「施工时定的」第 13 条）。
    Whole,
    /// 一段：从 `start` 到 `end`，两头都算。
    Part { start: u64, end: u64 },
    /// 超出：416。
    Beyond,
}

/// 照 `Range` 头和资源的大小 `size` 算要哪一段。只认一段：`bytes=a-b`、`bytes=a-`、`bytes=-n`。
pub(crate) fn span(header: Option<&str>, size: u64) -> Span {
    let Some(spec) = header.and_then(|header| {
        header
            .get(..6)
            .filter(|unit| unit.eq_ignore_ascii_case("bytes="))
            .map(|_| header[6..].trim())
    }) else {
        return Span::Whole;
    };
    // 好几段的（`a-b,c-d`）：逗号落在哪一边，那一边都不是数，照没写。
    let Some((first, last)) = spec.split_once('-') else {
        return Span::Whole;
    };
    let number = |text: &str| {
        (!text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| text.parse::<u64>().ok())
            .flatten()
    };
    match (first.trim(), last.trim()) {
        ("", suffix) => match number(suffix) {
            None => Span::Whole,
            Some(0) => Span::Beyond,
            Some(_) if size == 0 => Span::Beyond,
            Some(count) => Span::Part {
                start: size.saturating_sub(count),
                end: size - 1,
            },
        },
        (start, "") => match number(start) {
            None => Span::Whole,
            Some(start) if start >= size => Span::Beyond,
            Some(start) => Span::Part {
                start,
                end: size - 1,
            },
        },
        (start, end) => match (number(start), number(end)) {
            (Some(start), Some(end)) if start <= end => {
                if start >= size {
                    Span::Beyond
                } else {
                    Span::Part {
                        start,
                        end: end.min(size - 1),
                    }
                }
            }
            _ => Span::Whole,
        },
    }
}

/// `Content-Disposition`：叫浏览器存下来，名字只留最后一段（`/`、`\` 后面的），照 RFC 5987 转义。没有名字的只写
/// `attachment`。
pub(crate) fn attachment(name: Option<&str>) -> String {
    let last = name
        .and_then(|name| name.rsplit(['/', '\\']).next())
        .filter(|last| !last.is_empty());
    match last {
        None => "attachment".to_string(),
        Some(last) => format!("attachment; filename*=UTF-8''{}", escape(last)),
    }
}

/// RFC 5987 的 `attr-char` 照原样，别的字节写成 `%XX`。
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

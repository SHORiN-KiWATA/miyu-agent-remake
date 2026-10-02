//! 读回应的身子（`net.md`「怎么走」第 6、8 条，照桥的 `fetch.rs`）：页面读到 `</head>` 或者 `<body` 就停；图读到上限
//! 多一个字节；照开头的魔数认图。
//!
//! - 页面的字节上限只是兜底：要的全在 `<head>` 里（旧版 256 KB 的死上限读不到 YouTube 的 og 标签：它们在第 70 万字节
//!   上，`<head>` 里塞满了内联脚本，旧版 09-09 实测）。
//! - 图只收认得出的五种（PNG、JPEG、GIF、WebP、ICO），对方说是什么类型不算。**不收 SVG**：它能带脚本，头会把这些
//!   字节当图显示出来。

use reqwest::Response;

/// 读不下去了：连接断了、超时了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Broken;

/// 读回应的前 `max` 个字节，读满就停。
pub(crate) async fn read_prefix(mut response: Response, max: usize) -> Result<Vec<u8>, Broken> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Broken)? {
        let room = max.saturating_sub(body.len());
        if chunk.len() >= room {
            body.extend_from_slice(&chunk[..room]);
            break;
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// 读到 `</head>` 或 `<body` 就停，最多 `max` 个字节：交回读到的、截在记号那里的一截。
pub(crate) async fn read_head(mut response: Response, max: usize) -> Result<Vec<u8>, Broken> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Broken)? {
        // 记号可能跨在两块的接缝上：从上一块的尾巴开始找
        let scan_from = body.len().saturating_sub(MARKER_OVERLAP);
        let room = max.saturating_sub(body.len());
        let full = chunk.len() >= room;
        body.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if let Some(end) = find_head_end(&body[scan_from..]) {
            body.truncate(scan_from + end);
            break;
        }
        if full {
            break;
        }
    }
    Ok(body)
}

const HEAD_END: &[u8] = b"</head>";
const BODY_START: &[u8] = b"<body";
/// 接缝上往回看几个字节：长的那个记号的长度就够。
const MARKER_OVERLAP: usize = if HEAD_END.len() > BODY_START.len() {
    HEAD_END.len()
} else {
    BODY_START.len()
};

/// `<head>` 在哪儿结束：`</head>` 之后，或者 `<body` 之前，哪个先到算哪个（不分大小写）。
pub(crate) fn find_head_end(haystack: &[u8]) -> Option<usize> {
    let lower = haystack.to_ascii_lowercase();
    let head = lower
        .windows(HEAD_END.len())
        .position(|w| w == HEAD_END)
        .map(|at| at + HEAD_END.len());
    let body = lower
        .windows(BODY_START.len())
        .position(|w| w == BODY_START);
    match (head, body) {
        (Some(head), Some(body)) => Some(head.min(body)),
        (head, body) => head.or(body),
    }
}

/// 照开头的魔数认图，认出来的交回媒体类型；认不出的一律不要（分不清是什么就别当图交出去）。
///
/// 收 ICO 是因为一半的站 `rel=icon` 还是 `.ico`，不收就只能画首字母。
pub(crate) fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x00\x00\x01\x00") {
        Some("image/x-icon")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

#[cfg(test)]
mod tests;

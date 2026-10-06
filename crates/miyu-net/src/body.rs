//! 读回应的身子（`net.md`「怎么走」第 6、8 条，照桥的 `fetch.rs`）：页面读到 `</head>` 或者 `<body` 就停；图读到上限
//! 多一个字节；照开头的魔数认图。
//!
//! - 页面的字节上限只是兜底：要的全在 `<head>` 里（旧版 256 KB 的死上限读不到 YouTube 的 og 标签：它们在第 70 万字节
//!   上，`<head>` 里塞满了内联脚本，旧版 09-09 实测）。
//! - 图只收认得出的五种（PNG、JPEG、GIF、WebP、ICO），对方说是什么类型不算。**不收 SVG**：它能带脚本，头会把这些
//!   字节当图显示出来。
//! - 读到 `</head>`、`<body` 时，`<head>` 里要是什么都没挖到（没有 `<title>`、没有 `og:title`），接着往下读，只找
//!   `og:title`，找到就停（YouTube 把 `og:*` 放在 `</head>` 后面，W-7 补）。这个判断是边读边做的（[`HeadSignals`]），
//!   不会整段重扫：一个标签只收尾一次就处理一次，不管喂了多少次、喂的是不是从头开始的那一整截。
//! - YouTube 不照这一条在 `</head>` 停（[`Until::Video`]）：它的 `<head>` 里有 `<title>`，`og:*`、时长、频道名却都在
//!   `</head>` 后面。读到不空的 `og:title`、`itemprop="duration"`、`<link itemprop="name">` 三样都见到了为止，三样
//!   不齐的读到完或者读到上限（W-7 再补，`net.md`「怎么走」第 12 条）。
//! - `response.chunk()` 已经是解压过的字节（`miyu-net` 的客户端开着 gzip/brotli/deflate/zstd），这里的字节上限
//!   天然照解压以后的算：每次只多攒一块，攒够了就停，不会先把整个压缩炸弹解开再截。

use reqwest::Response;

use crate::html;

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

/// 页面读到哪儿为止。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Until {
    /// 普通网页：读到 `</head>`、`<body` 为止（`<head>` 里什么都没挖到的接着找 `og:title`）。
    Head,
    /// YouTube：读到 `og:title`、时长、频道名都见到了为止。
    Video,
    /// B 站的视频页：读到完（视频数据在页面后半截的脚本里，整页一二百 KB）。
    End,
}

/// 读页面，最多 `max` 个字节，读到哪儿为止照 `until`。
///
/// [`Until::Head`]：到了 `</head>`、`<body` 这个记号那里，要是已经看到够做卡片的东西（`<title>` 或者不空的
/// `og:title`）就在那儿截住，照旧；两个都没有的，不截，接着往下读，只找 `og:title`，找到就停（W-7 补，net.md
/// 「怎么走」第 6 条）。[`Until::Video`]：不看记号，三样都见到了就停；[`Until::End`]：读到完（W-7 再补）。
pub(crate) async fn read_head(
    mut response: Response,
    max: usize,
    until: Until,
) -> Result<Vec<u8>, Broken> {
    let mut body = Vec::new();
    let mut signals = HeadSignals::default();
    let mut marker = None;
    while let Some(chunk) = response.chunk().await.map_err(|_| Broken)? {
        // 记号可能跨在两块的接缝上：从上一块的尾巴开始找
        let scan_from = body.len().saturating_sub(MARKER_OVERLAP);
        let room = max.saturating_sub(body.len());
        let full = chunk.len() >= room;
        body.extend_from_slice(&chunk[..chunk.len().min(room)]);
        if until == Until::End {
            if full {
                break;
            }
            continue;
        }
        if until == Until::Video {
            signals.feed(&body, HeadSignals::video_done);
            if signals.video_done() || full {
                break;
            }
            continue;
        }
        if marker.is_none()
            && let Some(end) = find_head_end(&body[scan_from..]).map(|at| scan_from + at)
        {
            // 只喂到记号那儿：这一步的判断不能算上记号后面（正文）的标签
            signals.feed(&body[..end], HeadSignals::enough);
            if signals.enough() {
                body.truncate(end);
                break;
            }
            marker = Some(end);
        }
        signals.feed(&body, HeadSignals::enough);
        if marker.is_some() && signals.og_title {
            break;
        }
        if full {
            break;
        }
    }
    Ok(body)
}

/// 边读边记：`<title` 开了没有，不空的 `og:title` 有没有。扫完整的标签就把游标往前挪，没收尾的标签留着等
/// 下一截，不会整段重扫（W-7 补）。
#[derive(Debug, Default)]
struct HeadSignals {
    /// 扫到哪儿了：这之前的字节已经处理过，不会再看第二遍。
    scanned: usize,
    /// 看到 `<title` 了。
    title: bool,
    /// 看到不空的 `og:title` 了。
    og_title: bool,
    /// 看到不空的 `<meta itemprop="duration">` 了（YouTube）。
    duration: bool,
    /// 看到不空的 `<link itemprop="name">` 了（YouTube 的频道名）。
    author: bool,
}

impl HeadSignals {
    /// 够不够做卡片了：有 `<title>` 或者 `og:title` 就够。
    fn enough(&self) -> bool {
        self.title || self.og_title
    }

    /// YouTube 要的三样都见到了没有。
    fn video_done(&self) -> bool {
        self.og_title && self.duration && self.author
    }

    /// 喂目前攒到的全部字节（从头算，不是只新增的那一截）：处理这以后新收尾的标签，`done` 了就不再看。
    fn feed(&mut self, body: &[u8], done: fn(&HeadSignals) -> bool) {
        if done(self) {
            return;
        }
        while let Some(open) = body[self.scanned..].iter().position(|&b| b == b'<') {
            let start = self.scanned + open;
            let Some(close) = body[start..].iter().position(|&b| b == b'>') else {
                // 标签没收尾：等下一截，这次不往前挪游标
                break;
            };
            let end = start + close;
            let tag = &body[start + 1..end];
            self.scanned = end + 1;
            self.inspect(tag);
            if done(self) {
                break;
            }
        }
    }

    /// 一个收了尾的标签（不带尖括号）：是不是 `<title`，是不是不空的 `og:title`、时长、频道名。
    fn inspect(&mut self, tag: &[u8]) {
        let name_end = tag
            .iter()
            .position(|&b| b.is_ascii_whitespace() || b == b'/')
            .unwrap_or(tag.len());
        let name = &tag[..name_end];
        if name.eq_ignore_ascii_case(b"title") {
            self.title = true;
        } else if name.eq_ignore_ascii_case(b"meta") {
            let tag = String::from_utf8_lossy(tag);
            if html::meta_is_og_title(&tag) {
                self.og_title = true;
            } else if html::tag_value(&tag, "itemprop", "duration", "content").is_some() {
                self.duration = true;
            }
        } else if name.eq_ignore_ascii_case(b"link") {
            let tag = String::from_utf8_lossy(tag);
            if html::tag_value(&tag, "itemprop", "name", "content").is_some() {
                self.author = true;
            }
        }
    }
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

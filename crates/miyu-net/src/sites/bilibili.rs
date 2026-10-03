//! B 站（`net.md`「怎么走」第 12 条第 1 款，W-7 再补）：认出视频页（短链先跟完跳转再认），整页读完，照页面脚本里
//! `window.__INITIAL_STATE__` 的 `videoData` 取标题、简介、封面、UP 主、时长；取不到的照 `og:*` 和
//! `<meta name="author">`。`kind` 总是视频。
//!
//! 不调接口 `x/web-interface/view`：它对没登录的请求回 HTTP 412（带不带 `buvid3` 都一样，2026-10-03 项目主人实测），
//! 页面里的那份数据和接口给的是同一套字段（「起草时定的」第 14 条）。

use reqwest::Url;
use serde_json::Value;

use super::Draft;
use crate::Kind;
use crate::Why;
use crate::fetch::Fetcher;
use crate::html;
use crate::rules::Bilibili;

/// `url` 是不是 B 站的视频页：是的交回要读的那一页（短链跟完跳转以后落到的），不是的交 `None`。
///
/// # Errors
///
/// 短链跟跳转出了错：照那个错交（过不了闸、跳转太多是 [`Why::NoPreview`]，连不上是 [`Why::Unreachable`]）。
pub(super) async fn video_page(
    fetcher: &Fetcher,
    rules: &Bilibili,
    url: &Url,
) -> Result<Option<Url>, Why> {
    let page = if rules.short_hosts.matches(url) {
        fetcher.locate(url).await?
    } else {
        url.clone()
    };
    Ok((rules.hosts.matches(&page) && is_video(&page)).then_some(page))
}

/// 视频页：路径是 `/video/BV…`（10 个字母数字）或 `/video/av…`（数字），后面可以再跟别的段。
pub(super) fn is_video(url: &Url) -> bool {
    let Some(mut segments) = url.path_segments() else {
        return false;
    };
    if segments.next() != Some("video") {
        return false;
    }
    let id = segments.next().unwrap_or_default();
    if let Some(rest) = id.strip_prefix("BV") {
        return rest.len() == 10 && rest.bytes().all(|b| b.is_ascii_alphanumeric());
    }
    id.strip_prefix("av")
        .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
}

/// 照读到的整页补上视频的几格：脚本里的 `videoData` 有的照它，没有的照页面的 `og:*`、`<meta name="author">`。
pub(super) fn fill(text: &str, rules: &Bilibili, draft: &mut Draft) {
    draft.kind = Kind::Video;
    draft.site_fallback.clone_from(&rules.site);
    let Some(video) = video_data(text).as_ref().and_then(read) else {
        draft.author = html::first(text, "meta", "name", "author", "content").unwrap_or_default();
        return;
    };
    draft.found.title = video.title;
    if !video.description.trim().is_empty() {
        draft.found.description = video.description;
    }
    if let Some(picture) = video.picture.and_then(|picture| Url::parse(&picture).ok()) {
        draft.found.image = Some(picture);
    }
    draft.author = video.author;
    draft.duration = video.duration;
}

/// 页面脚本里 `window.__INITIAL_STATE__=` 后面那个 JSON 对象的 `videoData`；没有的、读不成的交 `None`。
pub(super) fn video_data(text: &str) -> Option<Value> {
    const MARK: &str = "__INITIAL_STATE__=";
    let start = text.find(MARK)? + MARK.len();
    // 后面跟着 `;(function…` 这些脚本：只读第一个完整的 JSON 值
    let state = serde_json::Deserializer::from_str(&text[start..])
        .into_iter::<Value>()
        .next()?
        .ok()?;
    state.get("videoData").cloned()
}

/// `videoData` 里取到的一个视频（原样的字）。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Video {
    pub(super) title: String,
    pub(super) description: String,
    pub(super) picture: Option<String>,
    pub(super) author: String,
    pub(super) duration: Option<u64>,
}

/// 读 `videoData`：`title` 不空的才算一个视频。
pub(super) fn read(data: &Value) -> Option<Video> {
    let text = |pointer: &str| {
        data.pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let title = text("/title");
    if title.trim().is_empty() {
        return None;
    }
    let picture = text("/pic");
    Some(Video {
        title,
        description: text("/desc"),
        picture: (!picture.trim().is_empty()).then_some(picture),
        author: text("/owner/name"),
        duration: data
            .pointer("/duration")
            .and_then(Value::as_u64)
            .filter(|seconds| *seconds > 0),
    })
}

#[cfg(test)]
mod tests;

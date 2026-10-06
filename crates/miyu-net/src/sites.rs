//! 按站取（`net.md`「怎么走」第 12、13 条，W-7 再补）：认得的站照自己的办法取，不认得的照普通网页。
//!
//! - B 站（`bilibili.rs`）：认出视频页，整页读完，照页面脚本里的视频数据取。
//! - YouTube（`youtube.rs`）：读页面读过 `</head>`，多挖时长、频道名。
//! - MediaWiki 站（`mediawiki.rs`）：先读页面认站，再调 `api.php` 补简介、图、站名。
//! - 人机验证页：头和状态码在 `fetch.rs` 读页面时认，标题在这里认；认出来的都是 `no_preview`。
//!
//! 站的请求都经 `fetch.rs`：同一道闸、同一套代理、复用的客户端。这里交回原样的字，收拢、截断、退到主机名在
//! [`finish`]，接口取到的字和页面挖到的走同一道。

mod bilibili;
mod mediawiki;
mod youtube;

use reqwest::Url;

use crate::body::Until;
use crate::fetch::Fetcher;
use crate::html::{self, Found};
use crate::rules::{Clip, Sites};
use crate::{Kind, Why};

/// 运行日志的目标。
const TARGET: &str = "miyu::net";

/// 一张卡片还没抓图的样子：挖到的、站补上的。
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Draft {
    /// 页面挖到的，或者站的接口取到的（原样的字）。
    pub(crate) found: Found,
    /// 是什么。
    pub(crate) kind: Kind,
    /// 视频多少秒。
    pub(crate) duration: Option<u64>,
    /// 作者：UP 主、频道名（原样的字）。
    pub(crate) author: String,
    /// 站名的退路：`og:site_name` 没有时先照它，再照主机名（MediaWiki 站的接口给的站名、标题的尾巴）。
    pub(crate) site_fallback: String,
}

/// 收拢、截好的字：卡片上直接用。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Texts {
    /// 标题，不是空的。
    pub(crate) title: String,
    /// 简介。
    pub(crate) description: String,
    /// 站名。
    pub(crate) site: String,
    /// 作者，没有的是 `None`。
    pub(crate) author: Option<String>,
}

/// 抓 `url`，照认得的站取：交回最后落到的那一页和还没抓图的卡片。
///
/// # Errors
///
/// 做不出卡片的：过不了闸、不是网页、人机验证页、B 站「视频没了」的页面（[`Why::NoPreview`]）；连不上、4xx、5xx
/// （[`Why::Unreachable`]）。
pub(crate) async fn draft(
    fetcher: &Fetcher,
    sites: &Sites,
    challenge_titles: &[String],
    url: &Url,
) -> Result<(Url, Draft), Why> {
    let bilibili = bilibili::video_page(fetcher, &sites.bilibili, url).await?;
    let target = bilibili.clone().unwrap_or_else(|| url.clone());
    let youtube = bilibili.is_none() && sites.youtube.matches(&target);
    let until = match (bilibili.is_some(), youtube) {
        (true, _) => Until::End,
        (false, true) => Until::Video,
        (false, false) => Until::Head,
    };
    let (page, text) = fetcher.page(&target, until).await?;
    let found = html::scan(&text, &page, youtube);
    let title = html::tidy(&found.title, usize::MAX);
    // 人机验证页；B 站「视频没了」的那一页（第 12 条第 1 款）
    let gone = bilibili.is_some() && sites.bilibili.gone_titles.contains(&title);
    if challenge_titles.contains(&title) || gone {
        return Err(Why::NoPreview);
    }
    let mut draft = Draft {
        found,
        ..Draft::default()
    };
    if bilibili.is_some() {
        bilibili::fill(&text, &sites.bilibili, &mut draft);
    } else if youtube {
        youtube::fill(&text, &mut draft);
    } else if mediawiki::is_mediawiki(&sites.mediawiki, &page, &text) {
        mediawiki::fill(fetcher, &sites.mediawiki, &page, &text, &mut draft).await;
    }
    Ok((page, draft))
}

/// 合成卡片上的字（第 7、14 条）：每一格去掉没填的模板、收拢、截断；站名先照 `og:site_name`，再照退路，最后是
/// 主机名。
///
/// # Errors
///
/// 没有标题：[`Why::NoPreview`]（不如留着原来的链接）。
pub(crate) fn finish(draft: &Draft, page: &Url, clip: &Clip) -> Result<Texts, Why> {
    let title = html::tidy(&draft.found.title, clip.title);
    if title.is_empty() {
        return Err(Why::NoPreview);
    }
    let site = [&draft.found.site, &draft.site_fallback]
        .into_iter()
        .map(|site| html::tidy(site, clip.site))
        .find(|site| !site.is_empty())
        .unwrap_or_else(|| html::tidy(&html::host_name(page), clip.site));
    let author = html::tidy(&draft.author, clip.author);
    Ok(Texts {
        title,
        description: html::tidy(&draft.found.description, clip.description),
        site,
        author: (!author.is_empty()).then_some(author),
    })
}

/// 接口没成、退回读页面：记一条（`net.md`「出错」）。只写主机名。
fn fell_back(site: &str, url: &Url) {
    tracing::warn!(
        target: TARGET,
        site,
        host = url.host_str().unwrap_or_default(),
        "link site fallback"
    );
}

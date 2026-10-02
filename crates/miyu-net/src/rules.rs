//! `resources/software/net/link_preview.json`：抓链接卡片的规矩，时限、上限、请求头、记多久、客户端留多久
//! （`net.md`「怎么走」第 10 条，施工 W-7）。多一格、少一格都不认，数不许是 0（「起草时定的」第 6 条）。

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;

/// 在资源目录里的位置。
fn file(resources: &Path) -> PathBuf {
    resources
        .join("software")
        .join("net")
        .join("link_preview.json")
}

/// `link_preview.json` 的样子。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    remember: RememberFile,
    page: Budget,
    image: Budget,
    redirects: usize,
    user_agent: String,
    accept_language: String,
    clip: ClipFile,
    clients: ClientsFile,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RememberFile {
    found_seconds: u64,
    no_preview_seconds: u64,
    unreachable_seconds: u64,
    entries: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Budget {
    timeout_seconds: u64,
    max_bytes: usize,
    accept: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClipFile {
    title: usize,
    description: usize,
    site: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClientsFile {
    seconds: u64,
    keep: usize,
}

/// 抓一样东西（页面、图）的预算。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fetching {
    /// 一跳最多多久：连上、发出、读完都算在里面，解析主机名也照它。
    pub(crate) timeout: Duration,
    /// 最多读多少字节。
    pub(crate) max_bytes: usize,
    /// 请求头 `Accept`。
    pub(crate) accept: String,
}

/// 标题、简介、站名最多几个字。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Clip {
    /// 标题。
    pub(crate) title: usize,
    /// 简介。
    pub(crate) description: usize,
    /// 站名。
    pub(crate) site: usize,
}

/// 读好的一份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rules {
    /// 抓到了的记多久。
    pub(crate) found: Duration,
    /// `no_preview` 记多久。
    pub(crate) no_preview: Duration,
    /// `unreachable` 记多久。
    pub(crate) unreachable: Duration,
    /// 最多记几条，满了整个清空。
    pub(crate) entries: usize,
    /// 抓页面。
    pub(crate) page: Fetching,
    /// 抓图。
    pub(crate) image: Fetching,
    /// 最多跟几次跳转。
    pub(crate) redirects: usize,
    /// 请求头 `User-Agent`：装成浏览器，有的站对不认识的客户端不给 og 标签。
    pub(crate) user_agent: String,
    /// 请求头 `Accept-Language`。
    pub(crate) accept_language: String,
    /// 字最多几个。
    pub(crate) clip: Clip,
    /// 复用的客户端留多久。
    pub(crate) client_ttl: Duration,
    /// 最多留几个客户端，满了整个清空。
    pub(crate) client_keep: usize,
}

/// 这一份读不了，或者写法不对。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RulesError {
    file: PathBuf,
    why: String,
}

impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.why)
    }
}

impl std::error::Error for RulesError {}

/// 读 `resources` 下的 `software/net/link_preview.json`。
pub(crate) fn load(resources: &Path) -> Result<Rules, RulesError> {
    let path = file(resources);
    let bad = |why: String| RulesError {
        file: path.clone(),
        why,
    };
    let text = std::fs::read_to_string(&path).map_err(|error| bad(error.to_string()))?;
    let parsed: File = serde_json::from_str(&text).map_err(|error| bad(error.to_string()))?;
    let numbers = [
        ("remember.found_seconds", parsed.remember.found_seconds),
        (
            "remember.no_preview_seconds",
            parsed.remember.no_preview_seconds,
        ),
        (
            "remember.unreachable_seconds",
            parsed.remember.unreachable_seconds,
        ),
        ("remember.entries", parsed.remember.entries as u64),
        ("page.timeout_seconds", parsed.page.timeout_seconds),
        ("page.max_bytes", parsed.page.max_bytes as u64),
        ("image.timeout_seconds", parsed.image.timeout_seconds),
        ("image.max_bytes", parsed.image.max_bytes as u64),
        ("clip.title", parsed.clip.title as u64),
        ("clip.description", parsed.clip.description as u64),
        ("clip.site", parsed.clip.site as u64),
        ("clients.seconds", parsed.clients.seconds),
        ("clients.keep", parsed.clients.keep as u64),
    ];
    if let Some((name, _)) = numbers.iter().find(|(_, value)| *value == 0) {
        return Err(bad(format!("{name} 不能是 0")));
    }
    Ok(Rules {
        found: Duration::from_secs(parsed.remember.found_seconds),
        no_preview: Duration::from_secs(parsed.remember.no_preview_seconds),
        unreachable: Duration::from_secs(parsed.remember.unreachable_seconds),
        entries: parsed.remember.entries,
        page: fetching(parsed.page),
        image: fetching(parsed.image),
        redirects: parsed.redirects,
        user_agent: parsed.user_agent,
        accept_language: parsed.accept_language,
        clip: Clip {
            title: parsed.clip.title,
            description: parsed.clip.description,
            site: parsed.clip.site,
        },
        client_ttl: Duration::from_secs(parsed.clients.seconds),
        client_keep: parsed.clients.keep,
    })
}

fn fetching(budget: Budget) -> Fetching {
    Fetching {
        timeout: Duration::from_secs(budget.timeout_seconds),
        max_bytes: budget.max_bytes,
        accept: budget.accept,
    }
}

#[cfg(test)]
mod tests;

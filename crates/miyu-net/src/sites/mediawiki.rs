//! MediaWiki 站（`net.md`「怎么走」第 12 条第 3 款，W-7 再补）：维基百科、ArchWiki 这类。页面已经读过了（认站
//! 要看 `generator`），再调 `api.php` 补简介、缩略图、站名；没装 TextExtracts 的照 `action=parse` 的第一段。
//!
//! 维基百科也走 `api.php`，不走它的 REST：一条路认所有 MediaWiki 站，站名放在同一个请求里（「起草时定的」第 12
//! 条）。第一段不回头读页面的正文（第 13 条）。

use reqwest::Url;
use serde_json::Value;

use super::{Draft, fell_back};
use crate::Kind;
use crate::fetch::Fetcher;
use crate::html;
use crate::rules::Mediawiki;

/// 是不是 MediaWiki 站：主机列着的，或者 `<meta name="generator">` 照 `MediaWiki` 开头（不分大小写）。
pub(super) fn is_mediawiki(rules: &Mediawiki, page: &Url, text: &str) -> bool {
    rules.api_path(page).is_some()
        || html::first(text, "meta", "name", "generator", "content").is_some_and(|generator| {
            generator
                .get(..9)
                .is_some_and(|head| head.eq_ignore_ascii_case("mediawiki"))
        })
}

/// 补上接口给的几格；接口没给的照页面。`kind` 是文章。
pub(super) async fn fill(
    fetcher: &Fetcher,
    rules: &Mediawiki,
    page: &Url,
    text: &str,
    draft: &mut Draft,
) {
    draft.kind = Kind::Article;
    // 页面的标题带着「 - 站名」的尾巴：去掉，尾巴留着当站名的最后一层退路
    if let Some((title, tail)) = draft.found.title.rsplit_once(" - ") {
        draft.site_fallback = tail.trim().to_string();
        draft.found.title = title.trim().to_string();
    }
    let (Some(name), Some(api)) = (page_name(text), api(rules, page, text)) else {
        return;
    };
    let Some(answer) = fetcher.json(&query(&api, &name, rules.thumbnail)).await else {
        fell_back("mediawiki", page);
        return;
    };
    let Some(summary) = read_query(&answer) else {
        fell_back("mediawiki", page);
        return;
    };
    let description = match summary.extract {
        Some(extract) => Some(extract),
        // 没装 TextExtracts：第一段照 parse 回的正文找
        None => fetcher.json(&parse(&api, &name)).await.and_then(|answer| {
            answer
                .pointer("/parse/text")
                .and_then(Value::as_str)
                .and_then(first_paragraph)
        }),
    };
    if !summary.title.trim().is_empty() {
        draft.found.title = summary.title;
    }
    if let Some(description) = description.filter(|text| !text.trim().is_empty()) {
        draft.found.description = description;
    }
    if let Some(image) = summary.thumbnail.and_then(|source| page.join(&source).ok()) {
        draft.found.image = Some(image);
    }
    if !summary.sitename.trim().is_empty() {
        draft.site_fallback = summary.sitename;
    }
}

/// 页面名：`"wgPageName":"…"` 那个 JSON 字符串（MediaWiki 每一页都写在 `RLCONF` 里）。
pub(super) fn page_name(text: &str) -> Option<String> {
    const KEY: &str = "\"wgPageName\":\"";
    let start = text.find(KEY)? + KEY.len() - 1;
    // 找到收尾的引号：跳过转义的
    let mut escaped = false;
    let length = text[start + 1..].char_indices().find_map(|(at, c)| {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return Some(at);
        }
        None
    })?;
    let name: String = serde_json::from_str(&text[start..start + 1 + length + 1]).ok()?;
    (!name.is_empty()).then_some(name)
}

/// `api.php` 在哪：主机列着的照落到的那一页接上列着的路径；没列着的照 `<link rel="EditURI">` 去掉问号后面的。
pub(super) fn api(rules: &Mediawiki, page: &Url, text: &str) -> Option<Url> {
    let mut api = match rules.api_path(page) {
        Some(path) => {
            let mut api = page.clone();
            api.set_path(path);
            api
        }
        None => page
            .join(&html::first(text, "link", "rel", "EditURI", "href")?)
            .ok()?,
    };
    api.set_query(None);
    api.set_fragment(None);
    Some(api)
}

/// `action=query`：简介、缩略图、站名一个请求（第 12 条第 3 款第 4 项）。
fn query(api: &Url, name: &str, thumbnail: u32) -> Url {
    let mut url = api.clone();
    url.query_pairs_mut()
        .append_pair("action", "query")
        .append_pair("format", "json")
        .append_pair("formatversion", "2")
        .append_pair("redirects", "1")
        .append_pair("prop", "extracts|pageimages")
        .append_pair("exintro", "1")
        .append_pair("explaintext", "1")
        .append_pair("piprop", "thumbnail")
        .append_pair("pithumbsize", &thumbnail.to_string())
        .append_pair("meta", "siteinfo")
        .append_pair("siprop", "general")
        .append_pair("titles", name);
    url
}

/// `action=parse`：只要第 0 节的正文（第 5 项）。
fn parse(api: &Url, name: &str) -> Url {
    let mut url = api.clone();
    url.query_pairs_mut()
        .append_pair("action", "parse")
        .append_pair("format", "json")
        .append_pair("formatversion", "2")
        .append_pair("redirects", "1")
        .append_pair("prop", "text")
        .append_pair("section", "0")
        .append_pair("page", name);
    url
}

/// `action=query` 读出来的（原样的字）。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Summary {
    pub(super) title: String,
    /// 没有这一格（站上没装 TextExtracts）是 `None`。
    pub(super) extract: Option<String>,
    pub(super) thumbnail: Option<String>,
    pub(super) sitename: String,
}

/// 读 `action=query` 的回应：没有 `query.pages[0]`、页面不存在的读不成。
pub(super) fn read_query(answer: &Value) -> Option<Summary> {
    let page = answer.pointer("/query/pages/0")?;
    if page.get("missing").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let text = |value: Option<&Value>| value.and_then(Value::as_str).map(str::to_string);
    Some(Summary {
        title: text(page.get("title")).unwrap_or_default(),
        extract: text(page.get("extract")),
        thumbnail: text(page.pointer("/thumbnail/source")),
        sitename: text(answer.pointer("/query/general/sitename")).unwrap_or_default(),
    })
}

/// 正文里第一段去掉标签以后不空的 `<p>`：只认正文最外层的段落，框、表格、列表里的不算（ArchWiki 顶上的
/// 「Related articles」框里就有 `<p>`，2026-10-03 项目主人实测）。最外层是 `mw-parser-output` 那一层，没有它包着的是
/// 整段的最外层。`<sup>`、`<style>`、`<script>` 连里面的字一起去掉，实体解开，空白收拢。
pub(super) fn first_paragraph(html: &str) -> Option<String> {
    /// 装着段落的几种块：它们里面的 `<p>` 不是正文的段落。
    const BLOCKS: &[&str] = &[
        "div",
        "table",
        "ul",
        "ol",
        "dl",
        "blockquote",
        "figure",
        "aside",
        "section",
        "nav",
    ];
    let lower = html.to_ascii_lowercase();
    let mut depth = 0usize;
    let mut root = None;
    let mut cursor = 0;
    while let Some(at) = lower[cursor..].find('<') {
        let open = cursor + at;
        let close = lower[open..].find('>').map(|at| open + at + 1)?;
        cursor = close;
        let inside = &lower[open + 1..close - 1];
        let (closing, rest) = match inside.strip_prefix('/') {
            Some(rest) => (true, rest),
            None => (false, inside),
        };
        let name: String = rest
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if BLOCKS.contains(&name.as_str()) {
            if closing {
                depth = depth.saturating_sub(1);
            } else if !inside.ends_with('/') {
                depth += 1;
                if root.is_none() && inside.contains("mw-parser-output") {
                    root = Some(depth);
                }
            }
            continue;
        }
        if closing || name != "p" || depth != root.unwrap_or(0) {
            continue;
        }
        let end = lower[close..]
            .find("</p>")
            .map_or(lower.len(), |at| close + at);
        let text = plain(&html[close..end]);
        if !text.is_empty() {
            return Some(text);
        }
        cursor = end;
    }
    None
}

/// 一段 HTML 去掉标签剩下的字：几种连里面的字一起去掉，实体解开，空白收拢。
fn plain(html: &str) -> String {
    let mut kept = String::new();
    let lower = html.to_ascii_lowercase();
    let mut cursor = 0;
    while let Some(at) = lower[cursor..].find('<') {
        let open = cursor + at;
        kept.push_str(&html[cursor..open]);
        let Some(close) = lower[open..].find('>').map(|at| open + at + 1) else {
            cursor = html.len();
            break;
        };
        let name: String = lower[open + 1..close - 1]
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        cursor = close;
        if matches!(name.as_str(), "sup" | "style" | "script") {
            let end_tag = format!("</{name}");
            cursor = lower[close..]
                .find(&end_tag)
                .and_then(|at| lower[close + at..].find('>').map(|gt| close + at + gt + 1))
                .unwrap_or(html.len());
        }
    }
    kept.push_str(&html[cursor..]);
    html::decode_entities(&kept)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests;

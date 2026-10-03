//! 从 HTML 的 `<head>` 里挖卡片要的几样（`net.md`「怎么走」第 7 条，照桥的 `html.rs`，桥照的是旧版）。
//!
//! 不建 DOM：要的只有标题、简介、图、站名、图标，全在 `<head>` 的 `<meta>`、`<link>`、`<title>` 里；为几百字节把整页
//! 塞进解析器不值得。取的次序：
//!
//! - 标题、简介、图：`og:*` 最先（不管它在文档里排第几），没有的照 `twitter:*`，再没有的照 `<title>`、
//!   `<meta name=description>`；
//! - 站名：`og:site_name`，没有的由合卡片的一方退到主机名（`sites.rs`，MediaWiki 站中间还有两层）；
//! - 图标：`rel` 里有 `icon` 的，没有的照 `apple-touch-icon`，再没有的试 `/favicon.ico`；
//! - 相对地址照最后落到的那一页（跟完跳转）的地址算。
//!
//! 这里交回原样的字（实体解开、两头空白去掉）；收拢空白、截断、没填的模板（[`tidy`]）由合卡片的一方做，接口取到
//! 的字也走同一道（W-7 再补）。站要的那几样（时长、作者、`generator`、`EditURI`）用 [`first`] 找。
//!
//! 纯函数，好测；网络那半在 `fetch.rs`。

use reqwest::Url;

/// 一页挖出来的：原样的字（还没收拢、截断）；图和图标是算好的绝对地址（还没抓）。
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Found {
    /// 标题：空的就做不成卡片。
    pub(crate) title: String,
    /// 简介，可以是空的。
    pub(crate) description: String,
    /// `og:site_name`，没写的是空的。
    pub(crate) site: String,
    /// 卡片的图。
    pub(crate) image: Option<Url>,
    /// 站点的图标。
    pub(crate) icon: Option<Url>,
}

/// 挖一页：`html` 是读到的那一截，`page` 是最后落到的地址。`whole` 的整段都看，不在 `</head>` 停（YouTube，
/// `net.md`「怎么走」第 12 条）。
pub(crate) fn scan(html: &str, page: &Url, whole: bool) -> Found {
    let head = extract(html, whole);
    let image = if head.image.is_empty() {
        None
    } else {
        page.join(&head.image).ok()
    };
    // 没写图标的试默认位置；试不到就没有，页面画首字母
    let icon = page
        .join(if head.icon.is_empty() {
            "/favicon.ico"
        } else {
            &head.icon
        })
        .ok();
    Found {
        title: head.title,
        description: head.description,
        site: head.site_name,
        image,
        icon,
    }
}

/// 主机名去掉 `www.`：没有站名时的站名。
pub(crate) fn host_name(page: &Url) -> String {
    page.host_str()
        .unwrap_or_default()
        .trim_start_matches("www.")
        .to_string()
}

/// 卡片上的一格字：留着没填的模板（`{$0}` 这样）的当没有；空白收拢成一个空格；超过 `limit` 个字的截断，加一个
/// 省略号（`net.md`「怎么走」第 7、14 条）。
pub(crate) fn tidy(text: &str, limit: usize) -> String {
    if has_template(text) {
        return String::new();
    }
    clip_text(text, limit)
}

/// 空白收拢成一个空格；超过 `limit` 个字的截断，加一个省略号。
fn clip_text(text: &str, limit: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= limit {
        return text;
    }
    let mut out: String = text.chars().take(limit).collect();
    out.push('…');
    out
}

/// 有没有没填的模板：`{$` 加一个以上的数字加 `}`（W-7 再补：B 站删了的视频页，og 的简介就是这样）。
fn has_template(text: &str) -> bool {
    text.match_indices("{$").any(|(at, _)| {
        let rest = &text[at + 2..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        digits > 0 && rest[digits..].starts_with('}')
    })
}

/// `<head>` 里原样取到的几样（实体已解码、两头空白已去掉）。
#[derive(Debug, Default, PartialEq, Eq)]
struct Head {
    title: String,
    description: String,
    image: String,
    site_name: String,
    icon: String,
}

/// 扫一遍 `<head>`：`og:*` 直接填，别的先记着，扫完再按次序补空着的。`whole` 的不在 `</head>` 停。
fn extract(html: &str, whole: bool) -> Head {
    let mut head = Head::default();
    let (mut twitter_title, mut twitter_description, mut twitter_image) =
        (String::new(), String::new(), String::new());
    let (mut document_title, mut plain_description, mut apple_icon) =
        (String::new(), String::new(), String::new());
    // 只转 ASCII 的大小写：字节数不变，在 `lower` 里找到的位置拿回 `html` 里切也对得上
    let lower = html.to_ascii_lowercase();
    if let Some(open) = lower.find("<title")
        && let Some(close) = lower[open..].find("</title>").map(|at| open + at)
        && let Some(start) = html[open..close].find('>').map(|at| open + at + 1)
    {
        document_title = decode_entities(html[start..close].trim());
    }
    for (name, tag) in tags(html) {
        // `</head>` 之后是正文，正文里的 meta 和卡片无关——除非 `<head>` 里什么都没挖到：那是 `body.rs` 判断要
        // 接着往下读的时候（YouTube 把 `og:*` 放在 `</head>` 后面），这时候就继续找，还是只看 meta、link
        // （W-7 补，net.md「怎么走」第 6 条）。
        if !whole
            && (name == "/head" || name == "body")
            && (!head.title.is_empty() || !document_title.is_empty())
        {
            break;
        }
        if name != "meta" && name != "link" {
            continue;
        }
        let attrs = attributes(tag);
        if name == "link" {
            let rel = attribute(&attrs, "rel")
                .unwrap_or_default()
                .to_ascii_lowercase();
            let href = attribute(&attrs, "href");
            if rel.split_whitespace().any(|value| value == "icon") {
                fill(&mut head.icon, href);
            } else if rel.contains("apple-touch-icon") {
                fill(&mut apple_icon, href);
            }
            continue;
        }
        let key = attribute(&attrs, "property")
            .or_else(|| attribute(&attrs, "name"))
            .unwrap_or_default()
            .to_ascii_lowercase();
        let content = attribute(&attrs, "content");
        match key.as_str() {
            "og:title" => fill(&mut head.title, content),
            "og:description" => fill(&mut head.description, content),
            "og:image" | "og:image:url" | "og:image:secure_url" => fill(&mut head.image, content),
            "og:site_name" => fill(&mut head.site_name, content),
            "twitter:title" => fill(&mut twitter_title, content),
            "twitter:description" => fill(&mut twitter_description, content),
            "twitter:image" | "twitter:image:src" => fill(&mut twitter_image, content),
            "description" => fill(&mut plain_description, content),
            _ => {}
        }
    }
    fill(&mut head.title, Some(&twitter_title));
    fill(&mut head.title, Some(&document_title));
    fill(&mut head.description, Some(&twitter_description));
    fill(&mut head.description, Some(&plain_description));
    fill(&mut head.image, Some(&twitter_image));
    fill(&mut head.icon, Some(&apple_icon));
    head
}

/// 一个个标签：（小写的标签名，尖括号里面的原文）。没收尾的标签到那儿为止。
fn tags(html: &str) -> impl Iterator<Item = (String, &str)> {
    let mut cursor = 0;
    std::iter::from_fn(move || {
        let start = cursor + html[cursor..].find('<')?;
        let length = html[start..].find('>')?;
        let tag = &html[start + 1..start + length];
        cursor = start + length + 1;
        let name = tag
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        Some((name, tag))
    })
}

/// 整段里第一个 `<tag key="value" …>` 的 `wanted` 那一格：站要的那几样用它找，例如
/// `first(html, "meta", "itemprop", "duration", "content")`（`net.md`「怎么走」第 12 条）。比法见 [`tag_value`]。
pub(crate) fn first(html: &str, tag: &str, key: &str, value: &str, wanted: &str) -> Option<String> {
    tags(html)
        .filter(|(name, _)| name == tag)
        .find_map(|(_, raw)| tag_value(raw, key, value, wanted))
}

/// 一个标签（尖括号里面的原文）的 `key` 是 `value`（不分大小写）的话，交回它 `wanted` 那一格（去掉两头空白，空的
/// 不算）。不看标签名：调的一方自己看。
pub(crate) fn tag_value(raw: &str, key: &str, value: &str, wanted: &str) -> Option<String> {
    let attrs = attributes(raw);
    let matched = attribute(&attrs, key).is_some_and(|got| got.eq_ignore_ascii_case(value));
    let got = attribute(&attrs, wanted).map(str::trim).unwrap_or_default();
    (matched && !got.is_empty()).then(|| got.to_string())
}

/// 已经有值的不覆盖：同一样东西写了好几遍的，文档里排在前面的算。
fn fill(slot: &mut String, value: Option<&str>) {
    if !slot.is_empty() {
        return;
    }
    if let Some(value) = value.map(str::trim)
        && !value.is_empty()
    {
        *slot = value.to_string();
    }
}

fn attribute<'a>(attributes: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// 这个 `<meta ...>` 标签（不带尖括号）是不是 `og:title`，内容不空：`body.rs` 边读边判断要不要接着往下读
/// 时用它（W-7 补，net.md「怎么走」第 6 条）。
pub(crate) fn meta_is_og_title(tag: &str) -> bool {
    let attrs = attributes(tag);
    let key = attribute(&attrs, "property")
        .or_else(|| attribute(&attrs, "name"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    key == "og:title" && attribute(&attrs, "content").is_some_and(|value| !value.trim().is_empty())
}

/// 一个标签的属性表：键转小写，值解码了常见的实体。引号可有可无，单双都认。
fn attributes(tag: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = tag.chars().collect();
    let mut out = Vec::new();
    // 跳过标签名
    let mut i = chars
        .iter()
        .position(|c| c.is_whitespace())
        .unwrap_or(chars.len());
    while i < chars.len() {
        while i < chars.len() && (chars[i].is_whitespace() || chars[i] == '/') {
            i += 1;
        }
        let start = i;
        while i < chars.len() && !chars[i].is_whitespace() && chars[i] != '=' && chars[i] != '/' {
            i += 1;
        }
        if i == start {
            break;
        }
        let key = chars[start..i]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase();
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() || chars[i] != '=' {
            out.push((key, String::new()));
            continue;
        }
        i += 1;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let value: String = if i < chars.len() && (chars[i] == '"' || chars[i] == '\'') {
            let quote = chars[i];
            let start = i + 1;
            i = start;
            while i < chars.len() && chars[i] != quote {
                i += 1;
            }
            let value = chars[start..i].iter().collect();
            i += 1;
            value
        } else {
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            chars[start..i].iter().collect()
        };
        out.push((key, decode_entities(&value)));
    }
    out
}

/// 一个实体最长看这么多字节去找分号：`&#x10FFFF;` 十个字节，命名的几个更短。只在 ASCII 字节里找分号，
/// 找到的位置一定落在字的边界上。
const ENTITY_WINDOW: usize = 12;

/// 只解常见的几个命名实体和数字实体：页面标题里出现的基本就这些；认不出的原样留着。
pub(crate) fn decode_entities(value: &str) -> String {
    if !value.contains('&') {
        return value.to_string();
    }
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let end = rest.bytes().take(ENTITY_WINDOW).position(|b| b == b';');
        let decoded = end.and_then(|end| {
            let text = match &rest[1..end] {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                "nbsp" => ' ',
                other => {
                    let digits = other.strip_prefix('#')?;
                    let code = match digits.strip_prefix(['x', 'X']) {
                        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                        None => digits.parse().ok()?,
                    };
                    char::from_u32(code)?
                }
            };
            Some((text, end))
        });
        match decoded {
            Some((text, end)) => {
                out.push(text);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests;

//! 从 HTML 的 `<head>` 里挖卡片要的几样（照旧版 `link_preview/html.rs`）。
//!
//! 不建 DOM：要的只有标题、简介、图、站名、图标，全在 `<head>` 的 `<meta>`、`<link>`、`<title>` 里；为几百字节把整页
//! 塞进解析器不值得。取的次序：
//!
//! - 标题、简介、图：`og:*` 最先（不管它在文档里排第几），没有的照 `twitter:*`，再没有的照 `<title>`、
//!   `<meta name=description>`；
//! - 站名：`og:site_name`，没有的用主机名（去掉 `www.`）；
//! - 图标：`rel=icon`，没有的照 `apple-touch-icon`，再没有的试 `/favicon.ico`；
//! - 相对地址照最后落到的那一页（跟完重定向）的地址算；字收拢空白，超出的截断加省略号。
//!
//! 纯函数，好测；网络那半在 `fetch.rs`。

use reqwest::Url;

/// 标题、简介、站名最多几个字（`resources/link_preview.json` 的 `clip`）。
pub(super) struct Clip {
    /// 标题。
    pub(super) title: usize,
    /// 简介。
    pub(super) description: usize,
    /// 站名。
    pub(super) site: usize,
}

/// 一页挖出来的：字已经收拢、截好；图和图标是算好的绝对地址（还没抓）。
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Found {
    /// 标题：空的就做不成卡片。
    pub(super) title: String,
    /// 简介，可以是空的。
    pub(super) description: String,
    /// 站名。
    pub(super) site: String,
    /// 卡片的图。
    pub(super) image: Option<Url>,
    /// 站点的图标。
    pub(super) icon: Option<Url>,
}

/// 挖一页：`html` 是读到 `</head>` 为止的那一截，`page` 是最后落到的地址。
pub(super) fn read(html: &str, page: &Url, clip: &Clip) -> Found {
    let head = extract(html);
    let site = if head.site_name.is_empty() {
        page.host_str().unwrap_or_default().trim_start_matches("www.").to_string()
    } else {
        head.site_name
    };
    let image = if head.image.is_empty() { None } else { page.join(&head.image).ok() };
    // 没写图标的试默认位置；试不到就没有，页面画首字母
    let icon = page.join(if head.icon.is_empty() { "/favicon.ico" } else { &head.icon }).ok();
    Found {
        title: clip_text(&head.title, clip.title),
        description: clip_text(&head.description, clip.description),
        site: clip_text(&site, clip.site),
        image,
        icon,
    }
}

/// 空白收拢成一个空格；超过 `limit` 个字的截断，加一个省略号。
pub(super) fn clip_text(text: &str, limit: usize) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= limit {
        return text;
    }
    let mut out: String = text.chars().take(limit).collect();
    out.push('…');
    out
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

/// 扫一遍 `<head>`：`og:*` 直接填，别的先记着，扫完再按次序补空着的。
fn extract(html: &str) -> Head {
    let mut head = Head::default();
    let (mut twitter_title, mut twitter_description, mut twitter_image) = (String::new(), String::new(), String::new());
    let (mut document_title, mut plain_description, mut apple_icon) = (String::new(), String::new(), String::new());
    // 只转 ASCII 的大小写：字节数不变，在 `lower` 里找到的位置拿回 `html` 里切也对得上
    let lower = html.to_ascii_lowercase();
    if let Some(open) = lower.find("<title")
        && let Some(close) = lower[open..].find("</title>").map(|at| open + at)
        && let Some(start) = html[open..close].find('>').map(|at| open + at + 1)
    {
        document_title = decode_entities(html[start..close].trim());
    }
    let mut cursor = 0;
    while let Some(offset) = lower[cursor..].find('<') {
        let start = cursor + offset;
        let Some(length) = html[start..].find('>') else {
            break;
        };
        let tag = &html[start + 1..start + length];
        cursor = start + length + 1;
        let name = tag.split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or_default().to_ascii_lowercase();
        // `</head>` 之后是正文，正文里的 meta 和卡片无关
        if name == "/head" || name == "body" {
            break;
        }
        if name != "meta" && name != "link" {
            continue;
        }
        let attrs = attributes(tag);
        if name == "link" {
            let rel = attribute(&attrs, "rel").unwrap_or_default().to_ascii_lowercase();
            let href = attribute(&attrs, "href");
            if rel.split_whitespace().any(|value| value == "icon") {
                fill(&mut head.icon, href);
            } else if rel.contains("apple-touch-icon") {
                fill(&mut apple_icon, href);
            }
            continue;
        }
        let key = attribute(&attrs, "property").or_else(|| attribute(&attrs, "name")).unwrap_or_default().to_ascii_lowercase();
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
    attributes.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
}

/// 一个标签的属性表：键转小写，值解码了常见的实体。引号可有可无，单双都认。
fn attributes(tag: &str) -> Vec<(String, String)> {
    let chars: Vec<char> = tag.chars().collect();
    let mut out = Vec::new();
    // 跳过标签名
    let mut i = chars.iter().position(|c| c.is_whitespace()).unwrap_or(chars.len());
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
        let key = chars[start..i].iter().collect::<String>().to_ascii_lowercase();
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
fn decode_entities(value: &str) -> String {
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
mod tests {
    use super::*;

    const CLIP: Clip = Clip { title: 120, description: 300, site: 60 };

    fn url(text: &str) -> Url {
        Url::parse(text).expect(text)
    }

    #[test]
    fn open_graph_wins_over_twitter_and_the_document_title() {
        // twitter 的排在前面也不算：og 先
        let html = r#"<html><head>
            <title>Fallback &amp; Co</title>
            <meta name="twitter:title" content="Twitter title">
            <meta name="twitter:image" content="/tw.png">
            <meta property="og:title" content="Register - Roogoo">
            <meta property="og:description" content="A borderless payment platform.">
            <meta property="og:image" content="/static/card.png">
            <meta property="og:site_name" content="Roogoo">
            <link rel="icon" href="/favicon.png">
        </head><body><meta property="og:title" content="正文里的不算"></body></html>"#;
        let found = read(html, &url("https://roogoo.example/register"), &CLIP);
        assert_eq!(found.title, "Register - Roogoo");
        assert_eq!(found.description, "A borderless payment platform.");
        assert_eq!(found.site, "Roogoo");
        assert_eq!(found.image, Some(url("https://roogoo.example/static/card.png")));
        assert_eq!(found.icon, Some(url("https://roogoo.example/favicon.png")));
    }

    #[test]
    fn twitter_comes_before_the_plain_tags() {
        let html = r#"<head>
            <title>Plain title</title>
            <meta name="description" content="plain description">
            <meta name="twitter:title" content="Twitter title">
            <meta name="twitter:description" content="twitter description">
            <meta name=twitter:image:src content=https://img.example/tw.jpg>
        </head>"#;
        let found = read(html, &url("https://example.com/"), &CLIP);
        assert_eq!(found.title, "Twitter title");
        assert_eq!(found.description, "twitter description");
        assert_eq!(found.image, Some(url("https://img.example/tw.jpg")));
    }

    #[test]
    fn falls_back_to_the_title_and_plain_description() {
        let html = r#"<head>
            <TITLE>Arch Wiki &#8212; Fcitx5</TITLE>
            <meta name="description" content='输入法配置'>
            <link rel="apple-touch-icon" href="touch.png">
        </head>"#;
        let found = read(html, &url("https://wiki.archlinux.org/title/Fcitx5"), &CLIP);
        assert_eq!(found.title, "Arch Wiki — Fcitx5");
        assert_eq!(found.description, "输入法配置");
        assert_eq!(found.image, None);
        assert_eq!(found.icon, Some(url("https://wiki.archlinux.org/title/touch.png")));
        // 没有 og:site_name 的用主机名
        assert_eq!(found.site, "wiki.archlinux.org");
    }

    #[test]
    fn relative_addresses_follow_the_final_page() {
        let html = r#"<head><title>x</title>
            <meta property="og:image" content="../img/card.png">
            <link rel="shortcut icon" href="//cdn.example.net/i.ico">
        </head>"#;
        let found = read(html, &url("https://www.example.com/a/b/page.html"), &CLIP);
        assert_eq!(found.image, Some(url("https://www.example.com/a/img/card.png")));
        assert_eq!(found.icon, Some(url("https://cdn.example.net/i.ico")));
        assert_eq!(found.site, "example.com");
        // 什么图标都没写的试 /favicon.ico
        let bare = read("<head><title>x</title></head>", &url("http://example.org:8080/deep/page"), &CLIP);
        assert_eq!(bare.icon, Some(url("http://example.org:8080/favicon.ico")));
    }

    #[test]
    fn text_is_collapsed_and_clipped() {
        let long = "字".repeat(CLIP.title + 10);
        let html = format!("<head><title>{long}</title><meta name=description content='  a\n\t b  '></head>");
        let found = read(&html, &url("https://example.com/"), &CLIP);
        assert_eq!(found.title.chars().count(), CLIP.title + 1);
        assert!(found.title.ends_with('…'));
        assert_eq!(found.description, "a b");
        assert_eq!(clip_text("  a\n  b  ", 10), "a b");
        assert_eq!(clip_text(&"字".repeat(10), 4), "字字字字…");
        let site = format!(r#"<head><title>x</title><meta property="og:site_name" content="{}"></head>"#, "s".repeat(80));
        assert_eq!(read(&site, &url("https://example.com/"), &CLIP).site.chars().count(), CLIP.site + 1);
    }

    #[test]
    fn entities_are_decoded() {
        assert_eq!(decode_entities("Tom &amp; Jerry"), "Tom & Jerry");
        assert_eq!(decode_entities("&lt;b&gt; &quot;q&quot; &apos;a&#39; &#x27;"), "<b> \"q\" 'a' '");
        assert_eq!(decode_entities("a&nbsp;b &#8212; &#X2014;"), "a b — —");
        // 认不出的、没有分号的原样留着
        assert_eq!(decode_entities("&notanentity; & &#xZZ; AT&T"), "&notanentity; & &#xZZ; AT&T");
        let html = r#"<head><meta property="og:title" content="R&amp;D &#x4E2D;文"></head>"#;
        assert_eq!(read(html, &url("https://example.com/"), &CLIP).title, "R&D 中文");
    }

    #[test]
    fn malformed_markup_does_not_panic() {
        for html in [
            "<meta property=og:title content=",
            "<<<>>><meta",
            "<link rel=icon href=",
            "<title>unclosed",
            "</title><title>closed before it opens",
            "&#xZZ; &notanentity; &",
            // 分号前面十几个字节全是多字节的字：找分号不能切在字中间
            "<meta property=og:title content='&中文中文中文中文中文'>",
            // 非 ASCII 的大写字母转小写会变长：位置不能错开
            "<title>İİİİ</title><meta property=og:title content=İ><link rel=icon href=/İ.png>",
        ] {
            let _found = read(html, &url("https://example.com/"), &CLIP);
        }
    }
}

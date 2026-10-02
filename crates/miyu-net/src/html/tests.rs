use super::*;

const CLIP: Clip = Clip {
    title: 120,
    description: 300,
    site: 60,
};

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
    assert_eq!(
        found.image,
        Some(url("https://roogoo.example/static/card.png"))
    );
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
    assert_eq!(
        found.icon,
        Some(url("https://wiki.archlinux.org/title/touch.png"))
    );
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
    assert_eq!(
        found.image,
        Some(url("https://www.example.com/a/img/card.png"))
    );
    assert_eq!(found.icon, Some(url("https://cdn.example.net/i.ico")));
    assert_eq!(found.site, "example.com");
    // 什么图标都没写的试 /favicon.ico
    let bare = read(
        "<head><title>x</title></head>",
        &url("http://example.org:8080/deep/page"),
        &CLIP,
    );
    assert_eq!(bare.icon, Some(url("http://example.org:8080/favicon.ico")));
}

#[test]
fn text_is_collapsed_and_clipped() {
    let long = "字".repeat(CLIP.title + 10);
    let html =
        format!("<head><title>{long}</title><meta name=description content='  a\n\t b  '></head>");
    let found = read(&html, &url("https://example.com/"), &CLIP);
    assert_eq!(found.title.chars().count(), CLIP.title + 1);
    assert!(found.title.ends_with('…'));
    assert_eq!(found.description, "a b");
    assert_eq!(clip_text("  a\n  b  ", 10), "a b");
    assert_eq!(clip_text(&"字".repeat(10), 4), "字字字字…");
    let site = format!(
        r#"<head><title>x</title><meta property="og:site_name" content="{}"></head>"#,
        "s".repeat(80)
    );
    assert_eq!(
        read(&site, &url("https://example.com/"), &CLIP)
            .site
            .chars()
            .count(),
        CLIP.site + 1
    );
}

#[test]
fn entities_are_decoded() {
    assert_eq!(decode_entities("Tom &amp; Jerry"), "Tom & Jerry");
    assert_eq!(
        decode_entities("&lt;b&gt; &quot;q&quot; &apos;a&#39; &#x27;"),
        "<b> \"q\" 'a' '"
    );
    assert_eq!(decode_entities("a&nbsp;b &#8212; &#X2014;"), "a b — —");
    // 认不出的、没有分号的原样留着
    assert_eq!(
        decode_entities("&notanentity; & &#xZZ; AT&T"),
        "&notanentity; & &#xZZ; AT&T"
    );
    let html = r#"<head><meta property="og:title" content="R&amp;D &#x4E2D;文"></head>"#;
    assert_eq!(
        read(html, &url("https://example.com/"), &CLIP).title,
        "R&D 中文"
    );
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

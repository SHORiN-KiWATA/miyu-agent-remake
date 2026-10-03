use serde_json::json;

use super::*;

fn url(text: &str) -> Url {
    Url::parse(text).expect(text)
}

fn rules() -> Mediawiki {
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    crate::rules::load(&resources).unwrap().sites.mediawiki
}

#[test]
fn a_listed_host_or_a_mediawiki_generator_is_mediawiki() {
    let rules = rules();
    assert!(is_mediawiki(
        &rules,
        &url("https://en.wikipedia.org/wiki/Rust"),
        ""
    ));
    let generator = r#"<meta name="generator" content="MediaWiki 1.43.1">"#;
    assert!(is_mediawiki(
        &rules,
        &url("https://wiki.example.org/x"),
        generator
    ));
    let lower = r#"<meta name="generator" content="mediawiki">"#;
    assert!(is_mediawiki(
        &rules,
        &url("https://wiki.example.org/x"),
        lower
    ));
    for other in [
        r#"<meta name="generator" content="WordPress 6.6">"#,
        r#"<meta name="generator" content="Media">"#,
        r#"<meta name="description" content="MediaWiki">"#,
        "",
    ] {
        assert!(
            !is_mediawiki(&rules, &url("https://wiki.example.org/x"), other),
            "{other}"
        );
    }
}

#[test]
fn the_page_name_is_the_json_string_in_rlconf() {
    let wikipedia = r#"<script>RLCONF={"wgBreakFrames":false,"wgPageName":"Rust_(programming_language)","wgTitle":"Rust"};</script>"#;
    assert_eq!(
        page_name(wikipedia).as_deref(),
        Some("Rust_(programming_language)")
    );
    // 转义的引号、斜杠、\u 都照 JSON 读；中文原样
    let escaped = r#""wgPageName":"Say_\"hi\"\/é_中文","wgTitle":"x""#;
    assert_eq!(page_name(escaped).as_deref(), Some("Say_\"hi\"/é_中文"));
    for missing in [
        "",
        r#""wgPageName":"""#,
        r#""wgPageName":"unterminated"#,
        r#""wgPageName":"bad\x""#,
    ] {
        assert_eq!(page_name(missing), None, "{missing}");
    }
}

#[test]
fn the_api_is_the_listed_path_or_the_edit_uri() {
    let rules = rules();
    assert_eq!(
        api(
            &rules,
            &url("https://zh.wikipedia.org/zh-cn/Rust?x=1#y"),
            ""
        )
        .map(String::from),
        Some("https://zh.wikipedia.org/w/api.php".to_string())
    );
    let edit = r#"<link rel="EditURI" type="application/rsd+xml" href="//wiki.example.org/w/api.php?action=rsd">"#;
    assert_eq!(
        api(&rules, &url("https://wiki.example.org/index.php/X"), edit).map(String::from),
        Some("https://wiki.example.org/w/api.php".to_string())
    );
    assert_eq!(
        api(&rules, &url("https://wiki.example.org/X"), "<head></head>"),
        None
    );
}

#[test]
fn the_query_answer_is_read() {
    let answer = json!({"batchcomplete": true, "query": {
        "pages": [{"pageid": 1, "ns": 0, "title": "Rust (programming language)",
            "extract": "Rust is a language.", "thumbnail": {"source": "https://upload.example/r.png", "width": 640}}],
        "general": {"sitename": "Wikipedia"}}});
    assert_eq!(
        read_query(&answer),
        Some(Summary {
            title: "Rust (programming language)".to_string(),
            extract: Some("Rust is a language.".to_string()),
            thumbnail: Some("https://upload.example/r.png".to_string()),
            sitename: "Wikipedia".to_string(),
        })
    );
    // 没装 TextExtracts、PageImages：没有那几格
    let bare = json!({"warnings": {}, "query": {"pages": [{"title": "Pacman"}], "general": {"sitename": "ArchWiki"}}});
    assert_eq!(read_query(&bare).and_then(|summary| summary.extract), None);
    assert_eq!(
        read_query(&json!({"query": {"pages": [{"title": "X", "missing": true}]}})),
        None
    );
    assert_eq!(read_query(&json!({"error": {"code": "badvalue"}})), None);
}

#[test]
fn the_first_paragraph_skips_empty_ones_and_drops_references() {
    // 只认正文最外层的段落：表格、框里的 <p> 不算（ArchWiki 顶上的「Related articles」框，2026-10-03 项目主人实测）
    let boxed = r#"<div class="mw-content-ltr mw-parser-output"><style>.x{}</style>
        <div class="archwiki-template-box archwiki-template-box-related"><p>Related articles</p>
        <ul><li><p>pacman/Tips and tricks</p></li></ul></div>
        <table><tr><td><p>infobox cell</p></td></tr></table>
        <p>The <b>pacman</b> package manager is one of the major features.</p></div>"#;
    assert_eq!(
        first_paragraph(boxed).as_deref(),
        Some("The pacman package manager is one of the major features.")
    );
    // 没有 mw-parser-output 那一层包着的，最外层就是最外层
    assert_eq!(
        first_paragraph("<div><p>boxed</p></div><p>top</p>").as_deref(),
        Some("top")
    );
    assert_eq!(
        first_paragraph("<table><tr><td><p>only boxed</p></td></tr></table>"),
        None
    );
    let wiki = r#"<div class="mw-parser-output"><p class="mw-empty-elt">
        </p><pre>code</pre><p><b>pacman</b> is a&nbsp;package
        manager<sup id="cite" class="reference"><a href="/n">[1]</a></sup>.<script>x()</script></p><p>second</p></div>"#;
    assert_eq!(
        first_paragraph(wiki).as_deref(),
        Some("pacman is a package manager.")
    );
    assert_eq!(first_paragraph("<p></p><p> <br> </p>"), None);
    assert_eq!(first_paragraph("no paragraphs"), None);
    // 坏的标记不崩
    for broken in [
        "<p",
        "<p>unclosed",
        "<p><sup>never closed",
        "<p><b",
        "<P>Upper</P>",
    ] {
        let _text = first_paragraph(broken);
    }
    assert_eq!(first_paragraph("<P>Upper</P>").as_deref(), Some("Upper"));
}

use super::*;

#[test]
fn only_real_image_bytes_are_kept() {
    for (bytes, kind) in [
        (
            &b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR"[..],
            Some("image/png"),
        ),
        (b"\xff\xd8\xff\xe0\x00\x10JFIF", Some("image/jpeg")),
        (b"GIF87a\x01\x00", Some("image/gif")),
        (b"GIF89a\x01\x00", Some("image/gif")),
        (b"RIFF\x24\x00\x00\x00WEBPVP8 ", Some("image/webp")),
        (b"\x00\x00\x01\x00\x01\x00\x10\x10", Some("image/x-icon")),
        // SVG 能带脚本：永远不收
        (
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"><script>alert(1)</script></svg>",
            None,
        ),
        (b"<?xml version=\"1.0\"?><svg/>", None),
        (b"<html><body>gotcha</body></html>", None),
        (b"<!DOCTYPE html>", None),
        (b"RIFF\x24\x00\x00\x00WAVEfmt ", None),
        // AVIF 不在收的五种里
        (b"\x00\x00\x00\x1cftypavif", None),
        (b"\x89PN", None),
        (b"\x00\x00\x02\x00", None),
        (b"", None),
    ] {
        assert_eq!(
            sniff_image(bytes),
            kind,
            "{}",
            String::from_utf8_lossy(bytes)
        );
    }
}

#[test]
fn head_signals_catch_a_title_tag() {
    let mut signals = HeadSignals::default();
    signals.feed(b"<html><head><title>Hi</title>");
    assert!(signals.enough());
    assert!(signals.title);
    assert!(!signals.og_title, "标题够了，不用管 og:title");
}

#[test]
fn head_signals_catch_a_non_empty_og_title() {
    let mut signals = HeadSignals::default();
    signals.feed(br#"<meta property="og:title" content="Hi">"#);
    assert!(signals.enough());
    assert!(signals.og_title);
}

#[test]
fn head_signals_ignore_an_empty_og_title() {
    let mut signals = HeadSignals::default();
    signals.feed(br#"<meta property="og:title" content="">"#);
    assert!(!signals.enough(), "内容是空的，不算够");
}

#[test]
fn head_signals_accept_name_and_reordered_attributes() {
    // name 是 property 的退路；属性先后不管
    let mut signals = HeadSignals::default();
    signals.feed(br#"<meta content="Hi" name="OG:TITLE">"#);
    assert!(signals.og_title, "大小写、先后都不该拦住");
}

#[test]
fn head_signals_do_not_rescan_a_tag_already_processed() {
    // 先喂一个不相干的标签，游标往前挪；再喂同一截 + 多出来的 og:title，只该看到新收尾的那个
    let mut signals = HeadSignals::default();
    signals.feed(b"<meta name=\"description\" content=\"d\">");
    assert!(!signals.enough());
    let scanned_after_first = signals.scanned;
    signals
        .feed(br#"<meta name="description" content="d"><meta property="og:title" content="Hi">"#);
    assert!(signals.og_title);
    assert!(
        signals.scanned > scanned_after_first,
        "游标该往前挪，不是从头再扫一遍"
    );
}

#[test]
fn head_signals_hold_an_unterminated_tag_for_the_next_feed() {
    // 标签跨在两次喂的接缝上：第一次没收尾不该算，第二次收了尾才算
    let mut signals = HeadSignals::default();
    signals.feed(br#"<meta property="og:titl"#);
    assert!(!signals.enough());
    signals.feed(br#"<meta property="og:title" content="Hi">"#);
    assert!(signals.og_title);
}

#[test]
fn a_head_is_cut_at_its_end() {
    let html = b"<html><head><title>x</title></head><body>aaaaaaaa</body></html>";
    assert_eq!(
        find_head_end(html).map(|end| &html[..end]),
        Some(&b"<html><head><title>x</title></head>"[..])
    );
    // 没有 </head> 的退到 <body 前面；大小写不管
    let no_close = b"<html><head><title>x</title><BODY>tail";
    assert_eq!(
        find_head_end(no_close).map(|end| &no_close[..end]),
        Some(&b"<html><head><title>x</title>"[..])
    );
    // 两个都有，先到的算
    let body_first = b"<head><body></head>";
    assert_eq!(find_head_end(body_first), Some(6));
    assert_eq!(find_head_end(b"<html><head><title>x</title>"), None);
}

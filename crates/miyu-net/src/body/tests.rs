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

use super::{MAX_BYTES, MAX_SIDE, fits, kind, measure};

#[test]
fn the_first_bytes_tell_the_kind() {
    assert_eq!(
        kind(b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0d"),
        Some("image/png")
    );
    assert_eq!(kind(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
    assert_eq!(kind(b"GIF87a\x01\x00"), Some("image/gif"));
    assert_eq!(kind(b"GIF89a\x01\x00"), Some("image/gif"));
    assert_eq!(kind(b"RIFF\x00\x00\x00\x00WEBP"), Some("image/webp"));
    // 差一点的都不算。
    for not in [
        &b"\x89PNG\r\n\x1a"[..],
        &[0xFF, 0xD8, 0x00],
        b"GIF88a",
        b"RIFF\x00\x00\x00\x00WAVE",
        b"RIFF\x00\x00\x00\x00WEB",
        b"BM\x00\x00",
        b"",
    ] {
        assert_eq!(kind(not), None, "{not:?}");
    }
}

/// 一张 `width` × `height` 的 PNG 的开头：签名和 IHDR，量宽高只看它。
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

#[test]
fn width_and_height_come_from_the_header() {
    assert_eq!(measure(&png(800, 600)), Some((800, 600)));
    assert_eq!(measure(&png(800, 600)[..12]), None, "截断了的量不出");
    assert_eq!(measure(b"hello"), None);
}

#[test]
fn only_pictures_within_both_limits_fit() {
    assert!(fits(MAX_BYTES, MAX_SIDE, MAX_SIDE), "正好在线上的收");
    assert!(!fits(MAX_BYTES + 1, 1, 1));
    assert!(!fits(1, MAX_SIDE + 1, 1));
    assert!(!fits(1, 1, MAX_SIDE + 1));
}

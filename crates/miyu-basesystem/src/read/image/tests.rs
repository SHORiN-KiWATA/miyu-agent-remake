use super::{kind, mib};

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

#[test]
fn sizes_are_written_in_mib_with_one_decimal() {
    assert_eq!(mib(5 * 1024 * 1024), "5.0 MiB");
    assert_eq!(mib(5 * 1024 * 1024 + 1), "5.0 MiB");
    assert_eq!(mib(7_654_321), "7.3 MiB");
}

//! 另配的头的模板（施工 8-14）：摘要怎么算、同一个种子同一个值，只认 `{session_digest}`。

use super::*;

/// 摘要是 SHA-256 的十六进制前 26 位（对照 `python3 -c "import hashlib; print(hashlib.sha256(b'ses-1').hexdigest()[:26])"`）。
#[test]
fn the_digest_is_the_first_26_hex_digits_of_sha256() {
    assert_eq!(digest("ses-1"), "09df36791e54c51f7be063867e");
    assert_eq!(digest(PROBE_SEED), "3dae4165d210945676f54e8946");
    assert_eq!(digest("ses-1"), digest("ses-1"), "同一个种子同一个值");
    assert_ne!(digest("ses-1"), digest("ses-2"));
}

#[test]
fn filling_replaces_every_digest_and_leaves_plain_values_alone() {
    assert_eq!(
        fill("ses_{session_digest}", "ses-1"),
        "ses_09df36791e54c51f7be063867e"
    );
    assert_eq!(
        fill("{session_digest}/{session_digest}", "ses-1"),
        "09df36791e54c51f7be063867e/09df36791e54c51f7be063867e"
    );
    assert_eq!(fill("cli", "ses-1"), "cli");
}

#[test]
fn only_the_session_digest_can_be_written_in_braces() {
    assert_eq!(check("ses_{session_digest}"), Ok(()));
    assert_eq!(check("cli"), Ok(()));
    for bad in [
        "msg_{call_digest}",
        "{version}",
        "ses_{session_digest",
        "ses_session_digest}",
        "{}",
    ] {
        let error = check(bad).expect_err(bad);
        assert!(error.contains("{session_digest}"), "{error}");
    }
}

//! 一行的样子：几列、什么时候加引号、来源去掉 `miyu::`、时刻到毫秒。

use super::*;

#[test]
fn a_line_has_the_columns_of_the_drawing() {
    let fields = [
        ("version", "0.0.0".to_string()),
        ("root", "~/.miyu".to_string()),
    ];
    let line = format(&Parts {
        time: "2026-09-27 21:03:15.284",
        level: &tracing::Level::INFO,
        source: "core",
        session: None,
        message: "started",
        fields: &fields,
    });
    assert_eq!(
        line,
        "2026-09-27 21:03:15.284 INFO  core     started version=0.0.0 root=~/.miyu"
    );
    let line = format(&Parts {
        time: "t",
        level: &tracing::Level::WARN,
        source: "session",
        session: Some("0199"),
        message: "rate limited",
        fields: &[
            ("status", "429".to_string()),
            ("retry", "1/5".to_string()),
            ("wait_ms", "3000".to_string()),
        ],
    });
    assert_eq!(
        line,
        "t WARN  session  0199 rate limited status=429 retry=1/5 wait_ms=3000"
    );
}

#[test]
fn values_are_quoted_only_when_needed_and_never_break_the_line() {
    assert_eq!(quote("deepseek-flash"), "deepseek-flash");
    assert_eq!(quote(""), "\"\"");
    assert_eq!(quote("a b"), "\"a b\"");
    assert_eq!(quote("say \"hi\""), "\"say \\\"hi\\\"\"");
    assert_eq!(quote("k=v"), "\"k=v\"");
    assert_eq!(quote("two\nlines"), "\"two\\nlines\"");
    assert_eq!(quote("C:\\x y"), "\"C:\\\\x y\"");
    assert_eq!(quote("a\tb"), "\"a\\tb\"");
    // 别的控制字符也加引号、写成十六进制：`cat` 日志时终端不会把它当成指令。
    assert_eq!(quote("bell\x07"), "\"bell\\x07\"");
    assert_eq!(escape("one\ntwo\r"), "one\\ntwo\\r");
    assert_eq!(escape("red\x1b[31m"), "red\\x1b[31m");
}

#[test]
fn the_session_and_the_message_never_break_the_line_either() {
    let line = format(&Parts {
        time: "t",
        level: &tracing::Level::INFO,
        source: "core",
        session: Some("s\nx"),
        message: "two\nlines\x1b[2J",
        fields: &[("model", "deepseek\nflash".to_string())],
    });
    assert_eq!(
        line,
        "t INFO  core     s\\nx two\\nlines\\x1b[2J model=\"deepseek\\nflash\""
    );
}

#[test]
fn our_targets_lose_the_prefix() {
    assert_eq!(source("miyu::http"), "http");
    assert_eq!(source("hyper::proto::h1"), "hyper::proto::h1");
}

#[test]
fn the_time_has_milliseconds() {
    let time = now();
    // 2026-09-27 21:03:15.284：日期、空格、时分秒、三位毫秒。
    assert_eq!(time.len(), 23, "{time}");
    assert_eq!(&time[10..11], " ");
    assert_eq!(&time[19..20], ".");
}

#[test]
fn the_utc_offset_is_hours_and_minutes() {
    let offset = utc_offset();
    let bytes = offset.as_bytes();
    assert_eq!(bytes.len(), 6, "{offset}");
    assert!(matches!(bytes[0], b'+' | b'-'), "{offset}");
    assert_eq!(bytes[3], b':', "{offset}");
    assert!(
        [1, 2, 4, 5].iter().all(|&i| bytes[i].is_ascii_digit()),
        "{offset}"
    );
}

//! 配置清单没有的几种写法（`chat.md` 第一条「守着它的」两种写法）：`rate` 的边界、`sleep` 的边界、`managers` 每一个的写法、
//! 编号的写法；写成别的类型的照 `wrong_type` 报。

use miyu_config::problem::Code;

use super::{id, manager, rate, sleep};
use crate::rules::Rules;
use crate::rules::test_support::system;

#[test]
fn rate_accepts_turns_per_duration_or_zero() {
    for good in [
        "0",
        "1/1s",
        "10000/86400s",
        "10000/24h",
        "10000/1440m",
        "30/60",
        "5/300s",
        "30/1m",
        "05/060s",
    ] {
        assert_eq!(rate(good), Ok(()), "{good}");
    }
}

#[test]
fn rate_turns_and_duration_have_bounds() {
    for (bad, code) in [
        ("0/60s", Code::OutOfRange),
        ("10001/60s", Code::OutOfRange),
        ("99999999999999999999/60s", Code::OutOfRange),
        ("5/86401s", Code::OutOfRange),
        ("5/25h", Code::OutOfRange),
        ("5/1441m", Code::OutOfRange),
        // 时长照配置清单的写法：`0`、`d` 这个单位都不是时长的写法。
        ("5/0s", Code::BadFormat),
        ("10000/1d", Code::BadFormat),
        ("5/2d", Code::BadFormat),
        ("5", Code::BadFormat),
        ("a/60s", Code::BadFormat),
        ("5/60x", Code::BadFormat),
        ("", Code::BadFormat),
        ("00", Code::BadFormat),
        ("/60s", Code::BadFormat),
        ("5/", Code::BadFormat),
        ("-1/60s", Code::BadFormat),
        ("+5/60s", Code::BadFormat),
        (" 5/60s", Code::BadFormat),
        ("5/60s/1", Code::BadFormat),
        ("５/60s", Code::BadFormat),
    ] {
        assert_eq!(rate(bad), Err(code), "{bad}");
    }
}

#[test]
fn sleep_accepts_spans_and_off() {
    for good in [
        "23:00-07:00",
        "07:00-23:00",
        "00:00-23:59",
        "23:59-00:00",
        "12:30-12:31",
        "off",
    ] {
        assert_eq!(sleep(good), Ok(()), "{good}");
    }
}

#[test]
fn sleep_rejects_bad_clock_times() {
    for (bad, code) in [
        ("24:00-07:00", Code::OutOfRange),
        ("07:00-24:00", Code::OutOfRange),
        ("23:60-07:00", Code::OutOfRange),
        ("99:99-07:00", Code::OutOfRange),
        ("7:00-8:00", Code::BadFormat),
        ("07:00-07:00", Code::BadFormat),
        ("00:00-00:00", Code::BadFormat),
        ("Off", Code::BadFormat),
        ("", Code::BadFormat),
        ("07:00", Code::BadFormat),
        ("07:00 - 08:00", Code::BadFormat),
        ("07:00-08:00-09:00", Code::BadFormat),
        ("0700-0800", Code::BadFormat),
        ("07:0a-08:00", Code::BadFormat),
        ("０7:00-08:00", Code::BadFormat),
    ] {
        assert_eq!(sleep(bad), Err(code), "{bad}");
    }
}

#[test]
fn manager_is_platform_colon_id() {
    for good in ["qq:10002", "tg:alice", "qq:1:2", "my-im_2:x"] {
        assert_eq!(manager(good), Ok(()), "{good}");
    }
    for bad in [
        "10002", "qq:", ":10002", "QQ:1", "1qq:1", "qq:1 2", "qq:1\t", "qq :1", "qq:\u{7}", "",
    ] {
        assert_eq!(manager(bad), Err(Code::BadFormat), "{bad}");
    }
}

#[test]
fn id_is_not_empty_and_has_no_blank_or_control() {
    assert!(id("123456") && id("abc") && id("-5"));
    assert!(!id("") && !id(" ") && !id("1 2") && !id("1\n") && !id("\u{3000}1"));
}

#[test]
fn forms_written_as_another_type_are_wrong_type() {
    for written in [
        "rate = 0",
        "sleep = false",
        "managers = \"qq:1\"",
        "managers = [10002]",
        "rate = [\"0\"]",
    ] {
        let read = Rules::parse(&[system("a.toml", &format!("[[rule]]\n{written}\n"))]);
        let codes: Vec<_> = read.problems.iter().map(|problem| problem.code).collect();
        assert_eq!(codes, [Code::WrongType], "{written}");
    }
    let read = Rules::parse(&[system("a.toml", "[[rule]]\nmanagers = []\n")]);
    assert!(read.problems.is_empty());
}

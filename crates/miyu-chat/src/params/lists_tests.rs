//! 出厂文件里清理的名单（`chat.md` 第八条「守着它的」清理的名单那一行，O-15 下）：空的列表照收；不可见字符每项正好一个
//! 字符（多了、空的、控制字符、写成别的）；标记 1 到 64 个字符；两份标记长度不同报收尾、只写一份报那一份另一份报缺、有一份
//! 写错的只报一次。场所规则那一边在 `rules/tables/tests.rs`。

use miyu_config::problem::Code;

use crate::Params;

use super::test_support::{DEFAULTS, bad, code_of, edited, file, line_of, missing, problems};

/// 出厂文件里开头那一份标记的那一行。
const LEAK_OPEN: &str = "leak_open = [\"<tool_call>\", \"<function=\"]";
/// 出厂文件里收尾那一份标记的那一行。
const LEAK_CLOSE: &str = "leak_close = [\"</tool_call>\", \"</function>\"]";

/// 出厂文件里 `invisible` 那一项（跨好几行）换成 `to`。
fn invisible(to: &str) -> String {
    let start = DEFAULTS.find("invisible = [").expect("invisible");
    let end = start + DEFAULTS[start..].find(']').expect("]") + 1;
    format!("{}{to}{}", &DEFAULTS[..start], &DEFAULTS[end..])
}

#[test]
fn the_cleaning_lists_may_be_empty() {
    let text = edited(LEAK_OPEN, "leak_open = []").replace(LEAK_CLOSE, "leak_close = []");
    let read = Params::read(&file(&text)).expect("空的标记照收");
    assert!(read.outbound.leak_open.is_empty() && read.outbound.leak_close.is_empty());
    let read = Params::read(&file(&invisible("invisible = []"))).expect("空的不可见字符照收");
    assert!(read.outbound.invisible.is_empty());
}

#[test]
fn the_invisible_list_takes_exactly_one_char_per_item() {
    let read = Params::read(&file(&invisible("invisible = [\"~\", \"\\u3000\"]")));
    assert_eq!(
        read.map(|p| p.outbound.invisible),
        Ok(vec!['~', '\u{3000}'])
    );
    for (to, code) in [
        ("invisible = [\"ab\"]", Code::BadFormat),
        ("invisible = [\"\"]", Code::BadFormat),
        // 控制字符：配置的文字不收。
        ("invisible = [\"\\u0007\"]", Code::BadFormat),
        ("invisible = [1]", Code::WrongType),
        ("invisible = \"\\u200B\"", Code::WrongType),
    ] {
        assert_eq!(code_of(&invisible(to), "outbound.invisible"), code, "{to}");
    }
}

#[test]
fn a_marker_is_one_to_64_chars() {
    let long = "汉".repeat(64);
    let text = edited(
        LEAK_OPEN,
        &format!("leak_open = [\"{long}\", \"<function=\"]"),
    );
    let read = Params::read(&file(&text)).map(|p| p.outbound.leak_open[0].chars().count());
    assert_eq!(read, Ok(64));
    for (to, code) in [
        (
            format!("leak_open = [\"{long}x\", \"<function=\"]"),
            Code::BadFormat,
        ),
        (
            "leak_open = [\"\", \"<function=\"]".to_string(),
            Code::BadFormat,
        ),
        ("leak_open = \"<tool_call>\"".to_string(), Code::WrongType),
    ] {
        // 写错的那一份报过，对上的那一份不再报（第八条「怎么走」第 7 条）。
        assert_eq!(
            code_of(&edited(LEAK_OPEN, &to), "outbound.leak_open"),
            code,
            "{to}"
        );
    }
}

#[test]
fn the_two_marker_lists_must_be_as_long_as_each_other() {
    let text = edited(LEAK_CLOSE, "leak_close = [\"</tool_call>\"]");
    let line = line_of(&text, "leak_close");
    let close = bad(
        Code::BadFormat,
        "outbound.leak_close",
        line,
        14,
        "[\"</tool_call>\"]",
    );
    assert_eq!(problems(&text), [close]);
    // 长的那一份是收尾也一样，报收尾。
    let text = edited(LEAK_OPEN, "leak_open = [\"<tool_call>\"]");
    let line = line_of(&text, "leak_close");
    let close = bad(
        Code::BadFormat,
        "outbound.leak_close",
        line,
        14,
        &LEAK_CLOSE[13..],
    );
    assert_eq!(problems(&text), [close]);
}

#[test]
fn a_marker_list_written_alone_is_reported_and_the_other_is_missing() {
    let text = edited(LEAK_CLOSE, "");
    let line = line_of(&text, "leak_open");
    let open = bad(
        Code::BadFormat,
        "outbound.leak_open",
        line,
        13,
        &LEAK_OPEN[12..],
    );
    assert_eq!(problems(&text), [open, missing("outbound.leak_close")]);
    let text = edited(LEAK_OPEN, "");
    let line = line_of(&text, "leak_close");
    let close = bad(
        Code::BadFormat,
        "outbound.leak_close",
        line,
        14,
        &LEAK_CLOSE[13..],
    );
    assert_eq!(problems(&text), [close, missing("outbound.leak_open")]);
}

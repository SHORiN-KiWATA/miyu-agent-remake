//! 时间线的一步一段（施工 9-8 上）：下一块开始了前面的算收全；几条命令里有一条出错不整行红；只开了调用就出错的
//! 拿掉；内核查出来的错只写分类的人话。

use miyu_kernel::event::ErrorClass;
use miyu_kernel::testkit::{Line, Play};
use miyu_view::{Body, Change};

use crate::support::*;

#[test]
fn a_thought_stops_when_the_next_block_starts() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("read", r#"{"file_path":"a"}"#)]).thinking("hmm"),
        Line::says("ok"),
    ]);
    stage.tools([Play::done("A")]);
    stage.say("read");
    let (_, changes) = live(&stage);
    let tool_added = changes
        .iter()
        .position(|c| matches!(c, Change::Add { entry, .. } if matches!(entry.body, Body::Tool(_))))
        .expect("加了工具那一步");
    let stopped = changes[..tool_added].iter().any(|c| {
        matches!(c, Change::Update { entry, .. } if matches!(&entry.body, Body::Thought(t) if !t.open))
    });
    assert!(stopped, "调工具那一块开始，思考就停表：{changes:#?}");
    same(&stage);
}

#[test]
fn one_failure_among_several_commands_does_not_red_the_whole_line() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[
                ("shell", r#"{"command":"ls","description":"List"}"#),
                ("shell", r#"{"command":"ls x","description":"List x"}"#),
            ],
        ),
        Line::says("ok"),
    ]);
    stage.tools([Play::done("a"), Play::Fails("no x".to_string())]);
    stage.say("list");
    let entries = same(&stage);
    let group = json(of_kind(&entries, "group")[0]);
    assert!(group.get("failed").is_none(), "{group}");
    assert_eq!(group["summary_en"][0]["text"], "Ran 2 commands · ");
    assert_eq!(group["summary_en"][1]["text"], "1 err");
}

#[test]
fn commands_lead_before_other_tools() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[
                ("read", r#"{"file_path":"a"}"#),
                ("shell", r#"{"command":"ls"}"#),
                ("shell", r#"{"command":"pwd"}"#),
            ],
        ),
        Line::says("ok"),
    ]);
    stage.tools([Play::done("a"), Play::done("b"), Play::done("c")]);
    stage.say("go");
    let entries = same(&stage);
    let summary = &json(of_kind(&entries, "group")[0])["summary_en"];
    let text = summary[0]["text"].as_str().unwrap_or_default();
    assert!(text.starts_with("Ran 2 commands · 1 tool · "), "{summary}");
}

#[test]
fn calls_that_failed_before_any_reply_are_taken_away() {
    let mut stage = stage();
    let mut broken = Line::breaks("", ErrorClass::Auth, "bad key");
    broken.calls = vec![("read".to_string(), r#"{"file_path":"a"}"#.to_string())];
    stage.model([broken]);
    stage.say("read");
    let (_, changes) = live(&stage);
    assert!(
        changes.iter().any(|c| matches!(c, Change::Remove { .. })),
        "{changes:#?}"
    );
    let entries = same(&stage);
    assert!(of_kind(&entries, "tool").is_empty());
}

#[test]
fn a_kernel_error_explains_with_its_class_only() {
    let mut stage = stage();
    stage.model((0..8).map(|_| Line::fails(ErrorClass::BadStream, "unexpected chunk")));
    stage.say("go");
    let entries = same(&stage);
    let end = json(of_kind(&entries, "end")[0]);
    assert_eq!(end["error"]["class"], "bad_stream", "{end}");
    assert_eq!(
        end["explain"], "Malformed response stream",
        "不写给运行日志的原话"
    );
}

#[test]
fn a_failing_command_beside_another_tool_does_not_red_the_whole_line() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[
                ("shell", r#"{"command":"ls x","description":"List x"}"#),
                ("read", r#"{"file_path":"a"}"#),
            ],
        ),
        Line::says("ok"),
    ]);
    stage.tools([Play::Fails("no x".to_string()), Play::done("a")]);
    stage.say("go");
    let entries = same(&stage);
    let group = json(of_kind(&entries, "group")[0]);
    assert!(
        group.get("failed").is_none(),
        "和别的工具同段的不整行红：{group}"
    );
    assert_eq!(group["summary_en"][0]["text"], "List x · 1 tool · ");
}

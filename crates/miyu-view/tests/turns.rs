//! 一轮：人说一句、她想了调工具再说话；收尾那一条；翻页和视图流最后一样（施工 9-8 上）。

use miyu_kernel::testkit::{Line, Play};

use crate::support::*;

#[test]
fn a_turn_with_a_thought_a_tool_and_a_reply() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("read", r#"{"file_path":"/home/alice/src/a.rs"}"#)])
            .thinking("look at a"),
        Line::says("Done."),
    ]);
    stage.tools([Play::done("fn main() {}")]);
    stage.say("read a");
    let entries = same(&stage);
    let kinds: Vec<String> = entries
        .iter()
        .map(|e| json(e)["kind"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        kinds,
        ["user", "group", "thought", "tool", "reply", "end"],
        "{entries:#?}"
    );
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(tool["state"], "ok");
    assert_eq!(tool["title"]["name"], "Read");
    assert_eq!(tool["title"]["object"], "~/src/a.rs");
    let group = json(of_kind(&entries, "group")[0]);
    assert_eq!(group["steps"].as_array().map(Vec::len), Some(2));
    assert!(group.get("open").is_none(), "说话了就收起：{group}");
    let first = group["summary_en"][0]["text"].as_str().unwrap_or_default();
    assert!(first.starts_with("Used 1 tool · 1 thought · "), "{group}");
    let end = json(of_kind(&entries, "end")[0]);
    assert_eq!(end["reason"], "completed");
}

#[test]
fn a_reply_before_the_calls_stands_between_two_groups() {
    let mut stage = stage();
    stage.model([
        Line::calls("Let me look.", &[("read", r#"{"file_path":"a"}"#)]),
        Line::calls("", &[("read", r#"{"file_path":"b"}"#)]),
        Line::says("Done."),
    ]);
    stage.tools([Play::done("A"), Play::done("B")]);
    stage.say("look");
    let entries = same(&stage);
    let kinds: Vec<String> = entries
        .iter()
        .map(|e| json(e)["kind"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        kinds,
        ["user", "reply", "group", "tool", "tool", "reply", "end"],
        "说话收起前面那一段，接着的步另起一段；两次请求之间没说话的还在同一段"
    );
}

#[test]
fn a_lone_command_leads_with_its_title_and_a_failure_reds_the_whole_line() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[("shell", r#"{"command":"ls /x","description":"List dirs"}"#)],
        ),
        Line::says("It failed."),
    ]);
    stage.tools([Play::Fails("no such dir".to_string())]);
    stage.say("list");
    let entries = same(&stage);
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(tool["state"], "error");
    assert_eq!(tool["title"]["object"], "List dirs", "命令写短标题");
    let group = json(of_kind(&entries, "group")[0]);
    assert_eq!(group["failed"], true, "只有这一条命令、它出错了：整行红");
    assert_eq!(group["summary_en"][0]["text"], "List dirs · ");
    assert_eq!(
        group["summary_en"][1],
        serde_json::json!({"text": "1 err", "tone": "error"})
    );
}

#[test]
fn an_edit_counts_its_lines() {
    let mut stage = stage();
    stage.model([
        Line::calls(
            "",
            &[(
                "edit",
                r#"{"file_path":"a.txt","edits":[{"old_string":"a\n","new_string":"b\nc\n"}]}"#,
            )],
        ),
        Line::says("Edited."),
    ]);
    stage.tools([Play::done("ok")]);
    stage.say("edit");
    let entries = same(&stage);
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(
        tool["diff"],
        serde_json::json!({"added": 2, "removed": 1, "estimated": true})
    );
    let summary = &json(of_kind(&entries, "group")[0])["summary_en"];
    assert_eq!(summary[0]["text"], "Made 1 edit ");
    assert_eq!(
        summary[1],
        serde_json::json!({"text": "+2", "tone": "added"})
    );
    assert_eq!(
        summary[3],
        serde_json::json!({"text": "-1", "tone": "removed"})
    );
}

#[test]
fn only_a_thought_folds_into_thought_for() {
    let mut stage = stage();
    stage.model([Line::says("ok").thinking("hmm")]);
    stage.say("think");
    let entries = same(&stage);
    let summary = &json(of_kind(&entries, "group")[0])["summary_en"];
    let text = summary[0]["text"].as_str().unwrap_or_default();
    assert!(text.starts_with("Thought for "), "{summary}");
    assert_eq!(
        summary.as_array().map(Vec::len),
        Some(1),
        "本来就带时间，不再接用时"
    );
}

#[test]
fn a_broken_reply_drops_the_calls_it_never_finished() {
    use miyu_kernel::event::ErrorClass;
    let mut stage = stage();
    let mut broken = Line::breaks("I'll read", ErrorClass::Auth, "bad key");
    broken.calls = vec![("read".to_string(), r#"{"file_path":"a"}"#.to_string())];
    stage.model([broken]);
    stage.say("read");
    let (_, changes) = live(&stage);
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, miyu_view::Change::Remove { .. })),
        "流式时开了的调用，落了盘没有，拿掉：{changes:#?}"
    );
    let entries = same(&stage);
    assert!(of_kind(&entries, "tool").is_empty());
    assert!(of_kind(&entries, "group").is_empty(), "空了的段也拿掉");
    let end = json(of_kind(&entries, "end")[0]);
    assert_eq!(end["reason"], "error");
    assert_eq!(end["error"]["class"], "auth");
    assert_eq!(end["explain"], "bad key", "供应商的原话照样写");
}

#[test]
fn an_interrupt_cancels_what_has_no_result() {
    use miyu_kernel::session::Queued;
    let mut stage = stage();
    stage.model([Line::calls(
        "",
        &[("shell", r#"{"command":"sleep 9","description":"Wait"}"#)],
    )]);
    stage.tools([Play::done("late").held()]);
    stage.say("wait");
    stage.interrupt(Queued::Send);
    let entries = same(&stage);
    let tool = json(of_kind(&entries, "tool")[0]);
    assert_eq!(tool["state"], "cancelled", "{entries:#?}");
    assert_eq!(json(of_kind(&entries, "end")[0])["reason"], "interrupted");
}

#[test]
fn ids_do_not_depend_on_where_feeding_starts() {
    use std::sync::Arc;
    let mut stage = stage();
    stage.model([Line::says("one")]);
    stage.say("first");
    let half = stage.log().len();
    stage.model([
        Line::calls("", &[("read", r#"{"file_path":"a"}"#)]),
        Line::says("two"),
    ]);
    stage.tools([Play::done("A")]);
    stage.say("second");
    let whole: Vec<String> = page(&stage).iter().map(|e| e.id.to_string()).collect();
    let mut projector = miyu_view::Projector::new(Arc::new(texts("en")), None);
    for event in &stage.log()[half..] {
        projector.event(event);
    }
    let tail: Vec<String> = projector
        .entries()
        .iter()
        .map(|e| e.id.to_string())
        .collect();
    assert!(!tail.is_empty());
    assert!(whole.ends_with(&tail), "{whole:?} {tail:?}");
}

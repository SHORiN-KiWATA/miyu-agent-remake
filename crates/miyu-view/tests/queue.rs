//! 排着的话（终端蓝图「运行状态行和排队的消息」第 5 条）：回答进行中来的先排着；被听到时挪到末尾、前面那段收起；
//! 被退回的不画（施工 9-8 上）。

use miyu_kernel::session::Queued;
use miyu_kernel::testkit::{Line, Play};
use miyu_view::Change;

use crate::support::*;

#[test]
fn a_message_sent_while_running_waits_then_moves_to_where_it_was_heard() {
    let mut stage = stage();
    stage.model([
        Line::calls("", &[("read", r#"{"file_path":"a"}"#)]),
        Line::says("ok"),
    ]);
    stage.tools([Play::done("A").held()]);
    stage.say("read a");
    stage.say("and b too");
    let (_, changes) = live(&stage);
    let queued = changes
        .iter()
        .any(|c| matches!(c, Change::Add { entry, .. } if json(entry)["queued"] == true));
    assert!(queued, "先排着：{changes:#?}");
    let call = stage.ran()[0].0;
    stage.release_tool(call);
    let entries = same(&stage);
    let kinds: Vec<String> = entries
        .iter()
        .map(|e| json(e)["kind"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        kinds,
        ["user", "group", "tool", "user", "reply", "end"],
        "{entries:#?}"
    );
    let heard = json(&entries[3]);
    assert_eq!(heard["text"], "and b too");
    assert!(heard.get("queued").is_none(), "听到了");
    assert_eq!(heard["turn"], json(&entries[0])["turn"], "归到这一轮");
    let (_, changes) = live(&stage);
    let moved = changes.iter().any(|c| {
        matches!(c, Change::Update { entry, after: Some(_) } if json(entry)["text"] == "and b too")
    });
    assert!(moved, "挪的那一下带位置");
}

#[test]
fn a_message_taken_back_is_not_drawn() {
    let mut stage = stage();
    stage.model([Line::calls("", &[("read", r#"{"file_path":"a"}"#)])]);
    stage.tools([Play::done("A").held()]);
    stage.say("read a");
    stage.say("never mind");
    stage.interrupt(Queued::Return);
    let entries = same(&stage);
    let users = of_kind(&entries, "user");
    let taken = json(users[1]);
    assert_eq!(taken["withdrawn"], true, "{entries:#?}");
    assert!(taken.get("queued").is_none());
}

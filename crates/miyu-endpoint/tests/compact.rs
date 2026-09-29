//! 手动压缩（施工 6-8，`docs/blueprint/protocol.md` 的 `session.compact`）：协议上压一次，回应是单开的那一轮的开头，
//! 人附的要求原样到了摘要请求里、记进了压缩，撤掉那一轮的回应里没有人说的话；有回合在进行、没有能压的，照头的语言
//! 拒绝；要求不是字的是参数不对。

mod support;

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, CompactTrigger};
use miyu_kernel::request::Message;
use miyu_session::testkit::{Play, Script};

use support::*;

/// 窗口大到不会自动压：第一轮说一句，第二次请求（摘要请求）交回一段摘要。出厂的尾巴是 16000 token，说得短的全在尾巴里，
/// 没有能压的：第一句说七万个字，一组就超了尾巴，压得掉它。
fn one_turn_then_a_summary() -> Script {
    Script::new([
        Play::Says("好。"),
        Play::Says("<analysis>a</analysis>\n<summary>\nThe user said hi.\n</summary>"),
    ])
    .window(1_000_000)
}

#[tokio::test]
async fn a_session_is_compacted_over_the_protocol() {
    let home = Home::new();
    let script = one_turn_then_a_summary();
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, &"x".repeat(70_000)).await;
    home.until_turns(&session, 1).await;
    let reply = client
        .call(
            "c3",
            "session.compact",
            json!({"session": session, "instructions": "keep the plan"}),
        )
        .await;
    home.until_turns(&session, 2).await;
    let log = home.log(&session);
    let started = log
        .iter()
        .rfind(|event| matches!(event.body, Body::TurnStarted(_)))
        .expect("开了一轮");
    assert_eq!(
        reply["result"],
        json!({"events": [started.seq.get()]}),
        "{reply}"
    );
    assert!(matches!(&started.body, Body::TurnStarted(opened) if opened.trigger.is_none()));
    assert_eq!(started.cause.as_ref().map(|id| id.as_str()), Some("c3"));
    let compacted = log
        .iter()
        .find_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted),
            _ => None,
        })
        .expect("压了");
    assert_eq!(compacted.trigger, Some(CompactTrigger::Manual));
    assert_eq!(compacted.instructions.as_deref(), Some("keep the plan"));
    assert_eq!(compacted.summary, "The user said hi.");
    // 摘要请求的最后一块：要求在「Additional Instructions:」那一行下面，最后一句还是不许调工具。
    let requests = script.requests();
    let Some(Message::User { blocks }) = requests[1].1.messages.last() else {
        panic!("摘要请求最后是人这边的指令");
    };
    let Some(Block::Text(text)) = blocks.last() else {
        panic!("指令是一块字");
    };
    assert!(
        text.text.contains(
            "\nAdditional Instructions:\nkeep the plan\n\nReply with the <analysis> block"
        ),
        "{}",
        text.text
    );
    assert!(text.text.ends_with("Do not call any tool.\n"));
    // 撤掉单开的那一轮：撤掉了一次压缩；那一轮没有触发，回应里没有人说的话。
    let reply = client
        .call("c4", "session.revert", json!({"session": session}))
        .await;
    let result = &reply["result"];
    assert_eq!(result["turns"], json!(1), "{reply}");
    assert_eq!(result["compactions"], json!(1), "{reply}");
    assert!(result.get("said").is_none(), "{reply}");
}

#[tokio::test]
async fn nothing_to_compact_is_refused_in_the_heads_language() {
    let home = Home::new();
    let core = home.core(&one_turn_then_a_summary());
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call("c2", "session.compact", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("nothing_to_compact"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        json!("没有能压的：还没压过的内容都在原样留着的最近一段里。")
    );
    let mut english = Client::connect(core);
    english.hello_without_input().await;
    let reply = english
        .call("e1", "session.compact", json!({"session": session}))
        .await;
    assert_eq!(
        reply["error"]["message"],
        json!(
            "Not enough to compact: everything not yet compacted is in the recent part that stays as it is."
        )
    );
    assert_eq!(home.log(&session).len(), 1, "拒绝的什么都不写");
}

#[tokio::test]
async fn a_running_turn_refuses_a_compaction() {
    let home = Home::new();
    let script = Script::new([Play::Holds]).window(1_000_000);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    until("请求停住", || script.requests().len() == 1).await;
    let reply = client
        .call("c3", "session.compact", json!({"session": session}))
        .await;
    assert_eq!(reason(&reply), Some("turn_running"), "{reply}");
    assert_eq!(
        reply["error"]["message"],
        json!("有回合在进行：先打断，或者等它做完。")
    );
}

#[tokio::test]
async fn instructions_that_are_not_text_are_bad_params() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call(
            "c2",
            "session.compact",
            json!({"session": session, "instructions": 7}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    let reply = client
        .call("c3", "session.compact", json!({"session": "nope"}))
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

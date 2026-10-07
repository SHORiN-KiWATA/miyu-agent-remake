//! 斜杠命令（施工 O-6，`docs/blueprint/protocol.md` 的 `command.run`）：真核心走一遍。`/clear`、别名 `/reset` 清空上下文，
//! `/stop` 打断这一轮（排着的话留着）、停掉子代理；执行了的记一条 `command.ran`，回执照连接的语言。认不出的回
//! `unknown_command`，不是 `/` 开头的参数不对；场所里只有主人、管理的人能用；内核拒了的照原因回、什么都不记；同一个编号只算一次。

mod support;

use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, ChildReason, EndReason, Event};
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use support::venues::bound_core;
use support::*;

async fn run(client: &mut Client, id: &str, params: Value) -> Value {
    client.call(id, "command.run", params).await
}

/// 日志里的 `command.ran`：命令名、原文、谁、起因。
fn ran(log: &[Event]) -> Vec<(String, String, By, String)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::CommandRan(ran) => Some((
                ran.command.clone(),
                ran.text.clone(),
                event.by.clone(),
                event
                    .cause
                    .as_ref()
                    .map(|id| id.as_str().to_string())
                    .unwrap_or_default(),
            )),
            _ => None,
        })
        .collect()
}

fn turns_started(log: &[Event]) -> usize {
    log.iter()
        .filter(|event| matches!(event.body, Body::TurnStarted(_)))
        .count()
}

#[tokio::test]
async fn clear_and_its_alias_clear_the_context_and_are_noted() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;

    let reply = run(
        &mut client,
        "k1",
        json!({"session": session, "text": "/clear"}),
    )
    .await;
    let log = home.log(&session);
    let compacted = log
        .iter()
        .find(|event| matches!(event.body, Body::ContextCompacted(_)))
        .expect("清空了");
    let noted = log
        .iter()
        .find(|event| matches!(event.body, Body::CommandRan(_)))
        .expect("记下了");
    assert_eq!(reply["result"]["command"], "clear", "{reply}");
    assert_eq!(reply["result"]["said"], "已清空上下文。");
    let events: Vec<u64> = serde_json::from_value(reply["result"]["events"].clone()).unwrap();
    assert_eq!(
        events.last(),
        Some(&noted.seq.get()),
        "最后一个是记下的那一条"
    );
    assert!(events[0] < compacted.seq.get(), "前面是清空那一轮的开头");
    assert_eq!(
        ran(&log),
        [(
            "clear".into(),
            "/clear".into(),
            By::Person(miyu_kernel::origin::Person::new(alice())),
            "k1/ran".into()
        )]
    );

    client.say("c3", &session, "再来").await;
    home.until_turns(&session, 3).await;
    // 别名、开头的空白、后面跟的字都认；记下的是原文。
    let reply = run(
        &mut client,
        "k2",
        json!({"session": session, "text": "  /reset 全部"}),
    )
    .await;
    assert_eq!(reply["result"]["command"], "clear", "{reply}");
    let commands: Vec<String> = ran(&home.log(&session))
        .into_iter()
        .map(|(_, text, _, _)| text)
        .collect();
    assert_eq!(commands, ["/clear", "  /reset 全部"]);
}

#[tokio::test]
async fn unknown_commands_and_plain_text_are_refused_and_nothing_is_noted() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let before = home.log(&session).len();
    for (id, text, expected) in [
        ("k1", "/nope", "unknown_command"),
        ("k2", "/", "unknown_command"),
        ("k3", "/ clear", "unknown_command"),
        ("k4", "clear", "bad_params"),
        ("k5", "", "bad_params"),
    ] {
        let reply = run(&mut client, id, json!({"session": session, "text": text})).await;
        assert_eq!(reason(&reply), Some(expected), "{text:?}：{reply}");
    }
    let reply = run(
        &mut client,
        "k6",
        json!({"session": session, "text": "/nope"}),
    )
    .await;
    assert_eq!(reply["error"]["message"], "没有这个命令。", "{reply}");
    let reply = run(
        &mut client,
        "k7",
        json!({"session": session, "text": "/clear", "extra": 1}),
    )
    .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    let reply = run(
        &mut client,
        "k8",
        json!({"session": "nope", "text": "/clear"}),
    )
    .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert!(ran(&home.log(&session)).is_empty());
    assert_eq!(home.log(&session).len(), before, "什么都没写");
}

/// 内核拒了的照它的原因回，`command.ran` 不记；同一个编号再发只算一次。
#[tokio::test]
async fn kernel_refusals_pass_through_and_the_same_id_counts_once() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = run(
        &mut client,
        "k1",
        json!({"session": session, "text": "/clear"}),
    )
    .await;
    assert_eq!(reason(&reply), Some("nothing_to_clear"), "{reply}");
    assert!(ran(&home.log(&session)).is_empty());

    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let first = run(
        &mut client,
        "k2",
        json!({"session": session, "text": "/clear"}),
    )
    .await;
    let again = run(
        &mut client,
        "k2",
        json!({"session": session, "text": "/clear"}),
    )
    .await;
    assert_eq!(again["result"], first["result"], "{again}");
    assert_eq!(ran(&home.log(&session)).len(), 1, "只记了一次");
    // 重启以后照样认得。
    let script = Script::new([]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let again = run(
        &mut client,
        "k2",
        json!({"session": session, "text": "/clear"}),
    )
    .await;
    assert_eq!(again["result"], first["result"], "{again}");
    assert_eq!(ran(&home.log(&session)).len(), 1);
}

/// `/stop`：这一轮打断，排着的那句留在日志里、不接着开；派出去的子代理也停了。没有在进行的回合也照样停、照样记。
#[tokio::test]
async fn stop_interrupts_keeps_the_queued_and_stops_subagents() {
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).unwrap()).unwrap();
    let args = json!({"description": "查 crate", "prompt": "Read Cargo.toml."}).to_string();
    let script = Script::new([
        Play::calls(&[("subagent", &args)]),
        Play::Holds,
        Play::Holds,
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools, TOKEN));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派一个去查").await;
    until("两次请求都停住", || script.requests().len() == 3).await;
    client.say("c3", &parent, "排着的").await;

    let reply = run(
        &mut client,
        "k1",
        json!({"session": parent, "text": "/stop"}),
    )
    .await;
    assert_eq!(reply["result"]["command"], "stop", "{reply}");
    assert_eq!(reply["result"]["said"], "已全部停下。");
    until("子代理停了", || {
        home.log(&parent)
            .iter()
            .any(|event| matches!(&event.body, Body::ChildReported(reported) if reported.reason == ChildReason::Stopped))
    })
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let log = home.log(&parent);
    let ended = log
        .iter()
        .find_map(|event| match &event.body {
            Body::TurnEnded(ended) => Some(ended.reason.clone()),
            _ => None,
        })
        .expect("那一轮结束了");
    assert_eq!(ended, EndReason::Interrupted);
    assert_eq!(turns_started(&log), 1, "排着的不接着开");
    assert_eq!(script.requests().len(), 3, "没有再发请求");
    let queued = log.iter().any(|event| match &event.body {
        Body::MessageUser(message) => format!("{:?}", message.blocks).contains("排着的"),
        _ => false,
    });
    assert!(queued, "排着的那句还在");
    assert!(
        !log.iter()
            .any(|event| matches!(event.body, Body::MessageWithdrawn(_))),
        "没有撤回"
    );
    assert_eq!(ran(&log).len(), 1);

    let reply = run(
        &mut client,
        "k2",
        json!({"session": parent, "text": "/stop"}),
    )
    .await;
    let noted = home.log(&parent).last().expect("有").seq.get();
    assert_eq!(
        reply["result"]["events"],
        json!([noted]),
        "没有回合在进行：只记一条：{reply}"
    );
}

/// 场所会话（`18-通讯平台.md` 第十二节）：主人、管理的人能用，别人 `command_not_allowed`；不带 `as` 的 `venue_session`，
/// 本机的会话带 `as` 的参数不对。
#[tokio::test]
async fn in_a_venue_only_the_owner_and_managers_may_run_commands() {
    let home = Home::new();
    let script = Script::new([Play::Says("在。")]);
    let mut client = Client::connect(bound_core(&home, &script, Catalog::default()));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let owner = json!({"external": "qq:10001"});
    let said = json!({"session": session, "text": "在吗", "as": owner});
    client.call("s1", "session.send", said).await;
    home.until_turns(&session, 1).await;

    let reply = run(
        &mut client,
        "k1",
        json!({"session": session, "text": "/stop"}),
    )
    .await;
    assert_eq!(reason(&reply), Some("venue_session"), "{reply}");
    let member = json!({"external": "qq:20002"});
    let reply = run(
        &mut client,
        "k2",
        json!({"session": session, "text": "/stop", "as": member}),
    )
    .await;
    assert_eq!(reason(&reply), Some("command_not_allowed"), "{reply}");
    assert_eq!(reply["error"]["message"], "只有主人和管理的人能用命令。");
    let member = json!({"external": "qq:20002", "role": "member"});
    let reply = run(
        &mut client,
        "k3",
        json!({"session": session, "text": "/clear", "as": member}),
    )
    .await;
    assert_eq!(reason(&reply), Some("command_not_allowed"), "{reply}");
    assert!(ran(&home.log(&session)).is_empty(), "被拒的什么都不记");

    let manager = json!({"external": "qq:10003", "role": "manager"});
    let reply = run(
        &mut client,
        "k4",
        json!({"session": session, "text": "/stop", "as": manager}),
    )
    .await;
    assert_eq!(reply["result"]["command"], "stop", "{reply}");
    let reply = run(
        &mut client,
        "k5",
        json!({"session": session, "text": "/clear", "as": owner}),
    )
    .await;
    assert_eq!(reply["result"]["command"], "clear", "{reply}");
    let who: Vec<String> = ran(&home.log(&session))
        .into_iter()
        .map(|(_, _, by, _)| match by {
            By::Person(person) => format!(
                "person via {:?}",
                person.via.map(|id| id.as_str().to_string())
            ),
            By::External(external) => format!("external {:?}", external.role),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        who,
        ["external Some(Manager)", "person via Some(\"qq:10001\")"]
    );

    let work = home.work.to_string_lossy().into_owned();
    let local = client.create("c1", &work).await;
    let reply = run(
        &mut client,
        "k6",
        json!({"session": local, "text": "/stop", "as": owner}),
    )
    .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}

/// `session.interrupt` 也收 `queued: "keep"`。
#[tokio::test]
async fn interrupt_takes_keep() {
    let home = Home::new();
    let script = Script::new([Play::Holds]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    until("请求停住", || script.requests().len() == 1).await;
    client.say("c3", &session, "排着的").await;
    let reply = client
        .call(
            "i1",
            "session.interrupt",
            json!({"session": session, "queued": "keep"}),
        )
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 1).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(turns_started(&home.log(&session)), 1, "排着的不接着开");
}

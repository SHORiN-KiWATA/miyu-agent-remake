//! 派子代理，真核心走一遍（施工 7-5，`docs/blueprint/agents.md` 第一条）：她在主会话里调 `agent`，会话表造出子会话，交代作为
//! 主会话发来的话开了它的第一轮，替身模型在子会话里答话；子会话的日志、快照、请求，`session.list` 的 `parent` 都对。

use serde_json::{Value, json};

use crate::support::{Client, Home, TOKEN, alice, default_resources};
use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Effect, Level, Permission};
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::{By, Session};
use miyu_kernel::request::Message;
use miyu_policy::Snapshot;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::Catalog;

/// 交代：两行，原样送到。
const PROMPT: &str = "Read Cargo.toml and say which crates the workspace has.\nOne line is enough.";

#[tokio::test]
async fn a_child_session_does_its_task_and_is_listed_under_its_parent() {
    let home = Home::new();
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).unwrap()).unwrap();
    let args = json!({"description": "查 crate", "prompt": PROMPT}).to_string();
    // 父会话的第二次请求和子会话的第一次请求谁先到不一定：两句一样，谁拿哪句都对。子会话答完向上回报，父会话由它开
    // 的那一轮是第四句（施工 7-6，回报一层层上来的在 `reports.rs`）。
    let script = Script::new([
        Play::calls(&[("subagent", &args)]),
        Play::Says("好。"),
        Play::Says("好。"),
        Play::Says("好。"),
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools, TOKEN));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派一个去查").await;
    home.until_turns(&parent, 1).await;

    let child = started_child(&home.log(&parent));
    home.until_turns(child.as_str(), 1).await;
    let log = home.log(child.as_str());
    let parent_id = SessionId::parse(&parent).unwrap();
    let from_parent = By::Session(Session {
        id: parent_id.clone(),
    });

    // 第一条：由父会话造，带着父会话和第几层，属主、场所、权限、工作目录照父会话的，不是一次性的。
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话");
    };
    assert_eq!(log[0].by, from_parent);
    assert_eq!(
        log[0]
            .cause
            .as_ref()
            .map(|cause| cause.as_str().to_string()),
        Some(format!("{parent}/j1"))
    );
    assert_eq!(created.parent, Some(parent_id.clone()));
    assert_eq!(created.depth, Some(1));
    assert_eq!(created.owner, alice());
    assert_eq!(created.venue.as_str(), "local");
    assert_eq!(
        created.permission,
        Permission {
            level: Level::Workspace,
            read_only: false
        }
    );
    assert_eq!(created.cwd.as_deref(), Some(work.as_str()));
    assert!(!created.oneshot);
    // 交代原样、作为父会话发来的话，开了它的第一轮。
    let Body::MessageUser(message) = &log[1].body else {
        panic!("第 2 条应该是交代：{:?}", log[1].body);
    };
    assert_eq!(log[1].by, from_parent);
    assert_eq!(
        message.blocks,
        [Block::Text(Text {
            text: PROMPT.to_string()
        })]
    );
    let Body::TurnStarted(turn) = &log[2].body else {
        panic!("第 3 条应该是开这一轮");
    };
    assert_eq!(turn.trigger, Some(log[1].seq));

    // 快照：软件工程师，有人能确认照父会话的，system 接上场所说明、再接核心的几行（施工 2-7 补），工具面照核心的目录。
    let bytes = Blobs::new(home.root.blobs(&alice()))
        .get(&created.policy)
        .unwrap();
    let snapshot = Snapshot::from_bytes(&bytes).unwrap();
    assert_eq!(snapshot.persona, "engineer");
    assert!(snapshot.attended);
    let read = |name: &str| std::fs::read_to_string(default_resources().join(name)).unwrap();
    let venue = read("core/jobs/subagent-venue.txt");
    let lines = read("core/permission-rule.txt") + &read("core/local-paths-rule.txt");
    assert!(
        snapshot
            .system
            .ends_with(&format!("{}\n\n{}", venue.trim_end(), lines.trim_end())),
        "{}",
        snapshot.system
    );
    assert!(
        snapshot.tools.iter().any(|tool| tool.name == "subagent"),
        "第 1 层还能派"
    );

    // 替身模型在子会话里答了：它那一次请求的最后一条就是交代。
    let asked = script
        .requests()
        .into_iter()
        .map(|(_, request)| request)
        .find(|request| request.system == snapshot.system)
        .expect("子会话请求过模型");
    let Some(Message::User { blocks, .. }) = asked.messages.last() else {
        panic!("最后一条是人这边的");
    };
    assert!(blocks.contains(&Block::Text(Text {
        text: PROMPT.to_string()
    })));

    // 会话列表：子会话写着父会话，主会话写 `null`。
    let listed = client.call("c3", "session.list", json!({})).await;
    let sessions = listed["result"]["sessions"].as_array().unwrap();
    let parent_of = |id: &str| -> Value {
        let item = sessions.iter().find(|item| item["session"] == id).unwrap();
        item["parent"].clone()
    };
    assert_eq!(parent_of(child.as_str()), json!(parent));
    assert_eq!(parent_of(&parent), Value::Null);
}

/// 父会话日志里 `job.started` 记着的子会话。
fn started_child(log: &[miyu_kernel::event::Event]) -> SessionId {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(&result.effects),
            _ => None,
        })
        .flatten()
        .find_map(|effect| match effect {
            Effect::JobStarted(started) => started.session.clone(),
            _ => None,
        })
        .expect("派出去了一个子会话")
}

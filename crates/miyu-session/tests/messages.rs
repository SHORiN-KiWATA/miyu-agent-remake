//! 父子之间留言，执行器这一头（施工 7-7，`docs/blueprint/agents.md` 第六条、`session/tools.md`「父子之间留言」）：会话表的
//! 端口换成假的，看 `message_agent` 的留言送对了会话、`by` 是这个会话、命令编号照调用、原话一块字；发给父的送到父会话；拒的
//! 每一种：主会话没有父、不是她派的（没派过、派它的那一轮撤掉了）、被停掉了、对方拒收、没有会话表。工具面上什么时候有
//! `message_agent`：本机的都有，到了深度上限的也有，群里没有。

mod support;

use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{
    Body, ChildReason, ChildReported, Effect, Event, JobMessaged, ToolResult,
};
use miyu_kernel::id::{CommandId, JobId, Seq, SessionId, TurnId, VenueId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::request::Request;
use miyu_kernel::session::{Command, Outcome, Reason};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Child, Handle, Lineage, Pending, SessionPort};
use miyu_tool::Catalog;

use support::*;

/// 假的会话表：造的第 n 个子会话编号末位是 n；记下发的命令，`refusing` 的回拒绝。
#[derive(Default)]
struct Table {
    made: Mutex<usize>,
    sent: Mutex<Vec<(SessionId, CommandId, By, Command)>>,
    refusing: bool,
}

impl Table {
    fn sent(&self) -> Vec<(SessionId, CommandId, By, Command)> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl SessionPort for Table {
    fn create(&self, _child: Child) -> Pending<'_, Result<SessionId, String>> {
        let mut made = self.made.lock().unwrap_or_else(PoisonError::into_inner);
        *made += 1;
        let answer = Ok(child_id(*made));
        Box::pin(async move { answer })
    }

    fn open(&self, _session: SessionId) -> Pending<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    /// 这几份假的会话表用不到读别的会话的日志（施工 C-4）。
    fn read_log(&self, _session: SessionId) -> Pending<'_, Result<miyu_tool::Log, String>> {
        Box::pin(async { Err("no logs here".to_string()) })
    }

    fn command(
        &self,
        session: SessionId,
        id: CommandId,
        by: By,
        command: Command,
    ) -> Pending<'_, Result<Outcome, String>> {
        // 交代照收；留言照 `refusing` 回。
        let message = id.as_str().contains("/message/");
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((session, id, by, command));
        let answer = match self.refusing && message {
            true => Outcome::Rejected {
                reason: Reason::Restoring,
            },
            false => Outcome::Accepted {
                events: vec![Seq::new(2).expect("从 1 数起")],
            },
        };
        Box::pin(async move { Ok(answer) })
    }
    fn stop(
        &self,
        _session: SessionId,
        _id: CommandId,
        _by: By,
    ) -> Pending<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    fn peek(&self, _session: SessionId) -> Pending<'_, Result<miyu_session::Peek, String>> {
        Box::pin(async { Ok(miyu_session::Peek::default()) })
    }

    fn sessions(
        &self,
        _owner: miyu_kernel::id::AccountId,
        _stop: miyu_tool::Stop,
    ) -> Pending<'_, Result<Vec<miyu_tool::MainSession>, String>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

/// 第 `n` 个子会话的编号。
fn child_id(n: usize) -> SessionId {
    SessionId::parse(&format!("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d9{n}")).expect("合写法")
}

/// 真的基础系统：`agent`、`message_agent` 在里面。
fn basesystem(home: &Home) -> Catalog {
    Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出")).expect("合写法")
}

/// 调一次 `agent`。
fn agent(title: &str) -> Play {
    let args = serde_json::json!({"description": title, "prompt": "Do it."});
    Play::calls(&[("subagent", &args.to_string())])
}

/// 调一次 `message_agent`。
fn message(to: &str, words: &str) -> Play {
    let args = serde_json::json!({"to": to, "message": words});
    Play::calls(&[("message_agent", &args.to_string())])
}

/// 造一个会话：场所、父会话照 `lines`，会话表是 `table`（没有的是空的）。
async fn session(home: &Home, script: &Script, table: Option<&Arc<Table>>, lines: Lines) -> Handle {
    let lines = Lines {
        sessions: table.map(|table| Arc::clone(table) as Arc<dyn SessionPort>),
        ..lines
    };
    home.create_full(script, &basesystem(home), Opening::default(), lines)
        .await
}

/// 父会话 `parent` 派出来的第 `depth` 层子会话。
fn child_of(parent: SessionId, depth: u32) -> Lines {
    Lines {
        lineage: Some(Lineage { parent, depth }),
        ..Lines::default()
    }
}

/// 发 `command`（编号 `id`、`by` 发的），等到磁盘上结束了 `turns` 轮。
async fn until_turns(
    home: &Home,
    handle: &Handle,
    id: &str,
    by: By,
    command: Command,
    turns: usize,
) -> Vec<Event> {
    within(
        "回应",
        handle.command(CommandId::parse(id).expect("命令编号合写法"), by, command),
    )
    .await
    .expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            == turns
    })
    .await
}

/// 说一句，等到结束了 `turns` 轮。
async fn one_turn(home: &Home, handle: &Handle, turns: usize) -> Vec<Event> {
    let id = format!("cmd-{turns}");
    until_turns(home, handle, &id, alice(), say("去做"), turns).await
}

/// 日志里最后一次工具结果的那一段字。
fn last_result(log: &[Event]) -> (&ToolResult, &str) {
    let result = log
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一次调用");
    match result.blocks.as_slice() {
        [Block::Text(Text { text })] => (result, text),
        other => panic!("一段字：{other:?}"),
    }
}

fn names(request: &Request) -> Vec<&str> {
    request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect()
}

#[tokio::test]
async fn a_message_reaches_the_subagent_as_the_parent_words() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        agent("甲"),
        Play::Says("派出去了。"),
        message("j1", "Use a.rs.\nNot b.rs."),
        Play::Says("告诉它了。"),
    ]);
    let handle = session(&home, &script, Some(&table), Lines::default()).await;
    one_turn(&home, &handle, 1).await;
    let log = one_turn(&home, &handle, 2).await;
    let (result, text) = last_result(&log);
    assert_eq!(text, "Message sent to j1.\n");
    assert_eq!(
        result.effects,
        [Effect::JobMessaged(JobMessaged {
            job: JobId::new(1).unwrap()
        })],
        "效果照原样记下：它欠一份回报"
    );
    let parent = handle.id().clone();
    let [_, (session, id, by, command)] = table.sent().try_into().expect("交代、留言各一次");
    assert_eq!(session, child_id(1), "送到 j1 的子会话");
    assert_eq!(
        id.as_str(),
        format!("{parent}/message/{}", result.call_id),
        "命令编号照这个会话、这次调用"
    );
    assert_eq!(
        by,
        By::Session(Session { id: parent }),
        "作为父会话发来的话"
    );
    assert_eq!(command, say("Use a.rs.\nNot b.rs."), "原话一块字");
}

#[tokio::test]
async fn a_subagent_at_the_depth_limit_messages_its_parent() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([message("parent", "Which file?"), Play::Says("问了。")]);
    let parent = child_id(9);
    let handle = session(&home, &script, Some(&table), child_of(parent.clone(), 2)).await;
    let log = one_turn(&home, &handle, 1).await;
    assert_eq!(last_result(&log).1, "Message sent to parent.\n");
    let [(session, _, by, command)] = table.sent().try_into().expect("留言一次");
    assert_eq!(session, parent, "送到父会话");
    assert_eq!(
        by,
        By::Session(Session {
            id: handle.id().clone()
        }),
        "作为子会话发来的话"
    );
    assert_eq!(command, say("Which file?"));
    // 到了深度上限：没有 `subagent`，`message_agent` 留着。
    let requests = script.requests();
    let tools = names(&requests[0].1);
    assert!(
        tools.contains(&"message_agent") && !tools.contains(&"subagent"),
        "{tools:?}"
    );
}

#[tokio::test]
async fn the_main_session_has_no_parent_and_a_stranger_is_not_hers() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        message("parent", "Hi."),
        Play::Says("好。"),
        message("j3", "Hi."),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, Some(&table), Lines::default()).await;
    let log = one_turn(&home, &handle, 1).await;
    assert_eq!(last_result(&log).1, "This session has no parent.\n");
    let log = one_turn(&home, &handle, 2).await;
    assert_eq!(
        last_result(&log).1,
        "\"j3\" is not a subagent you started. Message only your own subagents or your parent.\n"
    );
    assert!(table.sent().is_empty(), "一次都没送");
}

#[tokio::test]
async fn a_stopped_subagent_takes_no_more_messages() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        agent("甲"),
        Play::Says("派出去了。"),
        Play::Says("它被停了。"),
        message("j1", "Go on."),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, Some(&table), Lines::default()).await;
    one_turn(&home, &handle, 1).await;
    let stopped = ChildReported {
        job: JobId::new(1).unwrap(),
        session: child_id(1),
        reason: ChildReason::Stopped,
        text: String::new(),
        truncated: false,
        person: false,
        by_model: false,
    };
    let by = By::Session(Session { id: child_id(1) });
    let report = Command::Report(stopped);
    until_turns(&home, &handle, "report-1", by, report, 2).await;
    let log = one_turn(&home, &handle, 3).await;
    assert_eq!(
        last_result(&log).1,
        "Subagent j1 was stopped and takes no more messages.\n"
    );
    assert_eq!(table.sent().len(), 1, "只送过交代");
}

#[tokio::test]
async fn a_subagent_of_an_undone_turn_is_not_hers() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        agent("甲"),
        Play::Says("派出去了。"),
        message("j1", "Go on."),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, Some(&table), Lines::default()).await;
    let log = one_turn(&home, &handle, 1).await;
    let turn = log
        .iter()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| TurnId::new(event.seq))
        .expect("开过一轮");
    ask(&handle, "undo", Command::Revert { turn: Some(turn) })
        .await
        .unwrap();
    let log = one_turn(&home, &handle, 2).await;
    assert!(
        last_result(&log)
            .1
            .starts_with("\"j1\" is not a subagent you started."),
        "派它的那一轮撤掉了：她看不到它"
    );
    // 送过交代；撤销停它的回报经会话表送回父会话（施工 7-8）；留言一句都没送。
    let sent: Vec<String> = table
        .sent()
        .iter()
        .map(|(_, id, _, _)| id.as_str().to_string())
        .collect();
    assert!(
        sent.iter().all(|id| !id.contains("/message/")),
        "留言没送：{sent:?}"
    );
    assert!(sent.iter().any(|id| id.ends_with("/j1/undone")), "{sent:?}");
}

#[tokio::test]
async fn a_refused_message_or_no_table_is_not_delivered() {
    let home = Home::new();
    let table = Arc::new(Table {
        refusing: true,
        ..Table::default()
    });
    let script = Script::new([
        agent("甲"),
        Play::Says("派出去了。"),
        message("j1", "Go on."),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, Some(&table), Lines::default()).await;
    one_turn(&home, &handle, 1).await;
    let log = one_turn(&home, &handle, 2).await;
    assert_eq!(last_result(&log).1, "The message could not be delivered.\n");
    assert_eq!(table.sent().len(), 2, "送过一次，被拒");

    let script = Script::new([message("parent", "Hi."), Play::Says("好。")]);
    let handle = session(&home, &script, None, child_of(child_id(9), 1)).await;
    let log = one_turn(&home, &handle, 1).await;
    assert_eq!(
        last_result(&log).1,
        "The message could not be delivered.\n",
        "没有会话表的送不出去"
    );
}

#[tokio::test]
async fn every_local_session_has_it_and_a_group_does_not() {
    let home = Home::new();
    let group = Lines {
        venue: VenueId::parse("qq:group:123456").unwrap(),
        ..Lines::default()
    };
    for (lines, has) in [
        (Lines::default(), true),
        (child_of(child_id(9), 1), true),
        (group, false),
    ] {
        let script = Script::new([Play::Says("好。")]);
        let handle = session(&home, &script, None, lines).await;
        one_turn(&home, &handle, 1).await;
        let requests = script.requests();
        let request = &requests[0].1;
        assert_eq!(
            names(request).contains(&"message_agent"),
            has,
            "{:?}",
            names(request)
        );
    }
}

/// 载入的子会话照 `session.created` 记得自己的父：载入以后照样发得到父会话。
#[tokio::test]
async fn a_loaded_subagent_still_knows_its_parent() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        Play::Says("好。"),
        message("parent", "Which file?"),
        Play::Says("问了。"),
    ]);
    let parent = child_id(9);
    let handle = session(&home, &script, Some(&table), child_of(parent.clone(), 1)).await;
    one_turn(&home, &handle, 1).await;
    let id = handle.id().clone();
    stop(&handle).await;
    let port = Some(Arc::clone(&table) as Arc<dyn SessionPort>);
    let cwd = environment().cwd;
    let loaded = home
        .load_full(&id, &script, &basesystem(&home), &cwd, port)
        .await;
    let log = one_turn(&home, &loaded, 2).await;
    assert_eq!(last_result(&log).1, "Message sent to parent.\n");
    let [(session, _, _, _)] = table.sent().try_into().expect("留言一次");
    assert_eq!(session, parent);
}

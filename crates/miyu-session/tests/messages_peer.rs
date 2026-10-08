//! 发给别的会话，执行器这一头（施工 C-5，`docs/blueprint/cross-session.md` 第三条）：命令编号、`by` 照这个会话；没人看着
//! 的一次性会话交回 `held`；三种拒绝（限速、一模一样、对方没看的太多）对上三句。运行日志只记 `to`、不带话的字这一条，
//! 父子之间、别的会话走的是同一行代码，由 `messages_log.rs` 守着，这里不重复。两个主会话之间真的来回收发由
//! `crates/miyu-endpoint/tests/peers.rs` 守着，这里只测执行器自己翻译的那一段。

use std::sync::{Arc, Mutex, PoisonError};

use crate::support::{Home, Lines, Opening, ask, say};
use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome, Reason};
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Child, Handle, Pending, SessionPort};
use miyu_tool::MainSession;

/// 别的会话：短编号 `5a7c2d91`。
const OTHER: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

/// 假的会话表：往 `OTHER` 送的，照 `answer`、`held` 回；记下每一次发的命令。
struct Table {
    answer: Outcome,
    held: bool,
    sent: Mutex<Vec<(SessionId, CommandId, By, Command)>>,
}

impl Table {
    fn new(answer: Outcome, held: bool) -> Table {
        Table {
            answer,
            held,
            sent: Mutex::new(Vec::new()),
        }
    }

    fn sent(&self) -> Vec<(SessionId, CommandId, By, Command)> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl SessionPort for Table {
    fn create(&self, _child: Child) -> Pending<'_, Result<SessionId, String>> {
        Box::pin(async { Err("no children here".to_string()) })
    }

    fn open(&self, _session: SessionId) -> Pending<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    fn read_log(&self, _session: SessionId) -> Pending<'_, Result<miyu_tool::Log, String>> {
        Box::pin(async { Err("no logs here".to_string()) })
    }

    /// `OTHER` 照造它时给的 `held` 回；别的会话（这个测试没有）当不是。
    fn held(&self, session: SessionId) -> Pending<'_, bool> {
        let held = self.held && session.as_str() == OTHER;
        Box::pin(async move { held })
    }

    /// 这个测试不订「空了告诉我」（施工 C-6）。
    fn watch(
        &self,
        _session: SessionId,
        _watcher: SessionId,
        _since: miyu_kernel::time::Timestamp,
    ) -> Pending<'_, Result<(), miyu_session::NotWatched>> {
        Box::pin(async {
            Err(miyu_session::NotWatched::Failed(
                "no watches here".to_string(),
            ))
        })
    }

    fn command(
        &self,
        session: SessionId,
        id: CommandId,
        by: By,
        command: Command,
    ) -> Pending<'_, Result<Outcome, String>> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((session, id, by, command));
        let answer = self.answer.clone();
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

    /// 她能看到的唯一一个别的会话：`OTHER`。
    fn sessions(
        &self,
        _owner: AccountId,
        _stop: miyu_tool::Stop,
    ) -> Pending<'_, Result<Vec<MainSession>, String>> {
        let other = MainSession {
            id: SessionId::parse(OTHER).expect("合写法"),
            title: String::new(),
            cwd: "~/src".to_string(),
            busy: false,
            last_active: Timestamp::parse("2026-10-01T09:00:00.000Z").expect("合写法"),
        };
        Box::pin(async move { Ok(vec![other]) })
    }
}

/// 真的基础系统：`send_message`、`sessions` 在里面。
fn basesystem(home: &Home) -> miyu_tool::Catalog {
    miyu_tool::Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出"))
        .expect("合写法")
}

/// 调一次 `send_message`，`to` 写别的会话的整个编号。
fn message_to_other(words: &str) -> Play {
    let args = serde_json::json!({"to": OTHER, "message": words});
    Play::calls(&[("send_message", &args.to_string())])
}

/// 造一个会话：主会话，会话表是 `table`。
async fn session(home: &Home, script: &Script, table: &Arc<Table>) -> Handle {
    let lines = Lines {
        sessions: Some(Arc::clone(table) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    home.create_full(script, &basesystem(home), Opening::default(), lines)
        .await
}

/// 发完一句，等这一轮结束。
async fn one_turn(home: &Home, handle: &Handle, turns: usize) -> Vec<miyu_kernel::event::Event> {
    let id = format!("cmd-{turns}");
    ask(handle, &id, say("去发")).await.expect("会话在跑");
    crate::support::until_logged(home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, miyu_kernel::event::Body::TurnEnded(_)))
            .count()
            == turns
    })
    .await
}

/// 日志里最后一次工具结果的那一段字。
fn last_result(log: &[miyu_kernel::event::Event]) -> &str {
    use miyu_kernel::block::{Block, Text};
    use miyu_kernel::event::Body;
    log.iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .map(|result| match result.blocks.as_slice() {
            [Block::Text(Text { text })] => text.as_str(),
            other => panic!("一段字：{other:?}"),
        })
        .expect("有一次调用")
}

#[tokio::test]
async fn sending_to_another_session_uses_its_own_command_id_and_by() {
    let home = Home::new();
    let table = Arc::new(Table::new(Outcome::Accepted { events: Vec::new() }, false));
    let script = Script::new([message_to_other("去 CI 看一下"), Play::Says("发了。")]);
    let handle = session(&home, &script, &table).await;
    let log = one_turn(&home, &handle, 1).await;
    assert_eq!(last_result(&log), format!("Message sent to {OTHER}.\n"));
    let [(session, id, by, command)] = table.sent().try_into().expect("送过一次");
    assert_eq!(session, SessionId::parse(OTHER).unwrap());
    let this = handle.id().clone();
    assert_eq!(
        id.as_str(),
        format!("{this}/message/{}", last_call_id(&log)),
        "命令编号照这个会话、这次调用"
    );
    assert_eq!(
        by,
        By::Session(miyu_kernel::origin::Session { id: this }),
        "作为这个会话发来的话"
    );
    assert_eq!(command, say("去 CI 看一下"), "原话一块字");
}

#[tokio::test]
async fn a_held_session_says_so_and_a_watched_one_says_sent() {
    let home = Home::new();
    for (held, want) in [
        (
            true,
            format!(
                "Message saved for {OTHER}. Nobody is watching that one-shot session, so it reads this only when someone continues it.\n"
            ),
        ),
        (false, format!("Message sent to {OTHER}.\n")),
    ] {
        let table = Arc::new(Table::new(Outcome::Accepted { events: Vec::new() }, held));
        let script = Script::new([message_to_other("1"), Play::Says("好。")]);
        let handle = session(&home, &script, &table).await;
        let log = one_turn(&home, &handle, 1).await;
        assert_eq!(last_result(&log), want, "held={held}");
    }
}

#[tokio::test]
async fn the_three_peer_refusals_translate_to_their_own_sayings() {
    let home = Home::new();
    for (reason, want) in [
        (
            Reason::TooManyMessages,
            format!(
                "Too many messages to {OTHER} just now. Put the rest into one message and send it later.\n"
            ),
        ),
        (
            Reason::DuplicateMessage,
            format!("{OTHER} already has this exact message.\n"),
        ),
        (
            Reason::InboxFull,
            format!("{OTHER} has too many unread messages. Send again after it has read them.\n"),
        ),
    ] {
        let table = Arc::new(Table::new(Outcome::Rejected { reason }, false));
        let script = Script::new([message_to_other("1"), Play::Says("好。")]);
        let handle = session(&home, &script, &table).await;
        let log = one_turn(&home, &handle, 1).await;
        assert_eq!(last_result(&log), want, "{reason:?}");
    }
}

/// 日志里最后一次 `tool.result` 的调用编号（命令编号的组成部分）。
fn last_call_id(log: &[miyu_kernel::event::Event]) -> miyu_kernel::id::CallId {
    use miyu_kernel::event::Body;
    log.iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.call_id),
            _ => None,
        })
        .expect("有一次调用")
}

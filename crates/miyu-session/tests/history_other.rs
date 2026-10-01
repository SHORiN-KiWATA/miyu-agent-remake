//! `history` 读别的会话，执行器这一头（施工 C-4，`docs/blueprint/cross-session.md` 第二条、第九条）：真的会话
//! actor，`session` 参数认出来的另一个会话，读的是它磁盘上真实的日志，不载入它、在跑的也读得到；子会话没有列会话的
//! 端口，写了 `session` 直接拒。

mod support;

use std::sync::Arc;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{AccountId, CommandId, SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Child, Handle, Lineage, Pending, SessionPort};
use miyu_store::root::DataRoot;
use miyu_tool::{Catalog, Log, MainSession, Stop};

use support::*;

/// 假的会话表：只给列会话、读别的会话日志两样，都照 `root` 上真实的目录读（和生产里的会话表一样：不载入
/// 那个会话）。派子代理、发命令这几件这份测试用不到。
struct Table {
    root: DataRoot,
    listed: Vec<MainSession>,
}

impl SessionPort for Table {
    fn create(&self, _child: Child) -> Pending<'_, Result<SessionId, String>> {
        Box::pin(async { Err("no children here".to_string()) })
    }

    fn open(&self, _session: SessionId) -> Pending<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    fn command(
        &self,
        _session: SessionId,
        _id: CommandId,
        _by: By,
        _command: Command,
    ) -> Pending<'_, Result<Outcome, String>> {
        Box::pin(async { Err("not here".to_string()) })
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
        _owner: AccountId,
        _stop: Stop,
    ) -> Pending<'_, Result<Vec<MainSession>, String>> {
        let listed = self.listed.clone();
        Box::pin(async move { Ok(listed) })
    }

    /// 只算出会话 `session` 的真实目录（施工 C-4），和生产里的会话表一样不读盘、不载入它。
    fn read_log(&self, session: SessionId) -> Pending<'_, Result<Log, String>> {
        let dir = self.root.session_dir(&alice_account(), &session);
        Box::pin(async move { Ok(Log::new(LogDir(dir))) })
    }

    /// 这几个测试不发话（施工 C-5）：用不到。
    fn held(&self, _session: SessionId) -> Pending<'_, bool> {
        Box::pin(async { false })
    }
}

fn main(id: &SessionId) -> MainSession {
    MainSession {
        id: id.clone(),
        title: String::new(),
        cwd: "~/src".to_string(),
        busy: false,
        last_active: now(),
    }
}

/// 真的基础系统。
fn basesystem(home: &Home) -> Catalog {
    Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出")).expect("合写法")
}

/// 说一句，等到结束了 `turns` 轮。
async fn one_turn(home: &Home, handle: &Handle, turns: usize) -> Vec<Event> {
    let command = format!("cmd-{turns}");
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: "看看".to_string(),
        })],
        urgent: false,
    };
    within("回应", handle.command(id(&command), alice(), said))
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

/// 日志里最后一次工具调用的结果，原文。
fn last_result(log: &[Event]) -> String {
    let result = log
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一次调用");
    match result.blocks.as_slice() {
        [Block::Text(Text { text })] => text.clone(),
        other => panic!("一段字：{other:?}"),
    }
}

#[tokio::test]
async fn she_reads_another_running_sessions_real_log_without_loading_it() {
    let home = Home::new();

    // B：另一个主会话，先说一句，留下一份真实的日志。
    let script_b = Script::new([Play::Says("按会话分区，别再用一张大表了。")]);
    let b = home
        .create_full(
            &script_b,
            &basesystem(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    one_turn(&home, &b, 1).await;
    let b_id = b.id().clone();

    // A：另一个主会话，工具面上有 `sessions`、`history`；列会话的端口列得到 B，读日志照真实的数据根读。
    let table = Arc::new(Table {
        root: home.root.clone(),
        listed: vec![main(&b_id)],
    });
    let script_a = Script::new([
        Play::calls(&[("history", &format!(r#"{{"session":"{}"}}"#, b_id.short()))]),
        Play::Says("看到了。"),
    ]);
    let lines = Lines {
        sessions: Some(Arc::clone(&table) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    let a = home
        .create_full(&script_a, &basesystem(&home), Opening::default(), lines)
        .await;
    let log = one_turn(&home, &a, 1).await;
    assert!(
        last_result(&log).contains("按会话分区，别再用一张大表了。"),
        "{}",
        last_result(&log)
    );
    // B 没有被载入：这个场地里唯一认得会话的是 `Table`，A 读它全靠 `read_log` 直接开目录，不经 `open`/`command`。
}

#[tokio::test]
async fn a_child_session_cannot_use_the_session_parameter() {
    let home = Home::new();
    let parent = SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d99").expect("合写法");
    let child = Lines {
        lineage: Some(Lineage { parent, depth: 1 }),
        ..Lines::default()
    };
    let script = Script::new([
        Play::calls(&[("history", r#"{"session":"deadbeef"}"#)]),
        Play::Says("好。"),
    ]);
    let handle = home
        .create_full(&script, &basesystem(&home), Opening::default(), child)
        .await;
    let log = one_turn(&home, &handle, 1).await;
    assert_eq!(
        last_result(&log),
        "This session cannot read other sessions.\n"
    );
}

#[tokio::test]
async fn a_group_session_cannot_use_the_session_parameter() {
    let home = Home::new();
    let group = Lines {
        venue: VenueId::parse("qq:group:123456").expect("合写法"),
        ..Lines::default()
    };
    let script = Script::new([
        Play::calls(&[("history", r#"{"session":"deadbeef"}"#)]),
        Play::Says("好。"),
    ]);
    let handle = home
        .create_full(&script, &basesystem(&home), Opening::default(), group)
        .await;
    let log = one_turn(&home, &handle, 1).await;
    assert_eq!(
        last_result(&log),
        "This session cannot read other sessions.\n"
    );
}

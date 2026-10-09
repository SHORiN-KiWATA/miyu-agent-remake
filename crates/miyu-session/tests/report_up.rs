//! 向上回报，执行器这一头（施工 7-6，`docs/blueprint/agents.md` 第二条第 5 条、第八条）：会话表的端口换成假的，看子会话把
//! 回报交给父会话：命令编号照那一轮定、发命令的是子会话、任务编号照造它的命令读回；载入时最后那一份再交一次；父会话载入
//! 以后叫起还没回报的子会话。

use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{ChildReason, ChildReported};
use miyu_kernel::id::{CommandId, JobId, Seq, SessionId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::session::{Command, Outcome, Reason};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Child, Handle, Lineage, Pending, SessionPort};
use miyu_tool::Catalog;

use crate::support::*;

/// 父会话。
const PARENT: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";
/// 父会话派出来的子会话。
const CHILD: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

/// 假的会话表：造的子会话是 [`CHILD`]，记下发的命令、叫起的会话。
#[derive(Default)]
struct Table {
    sent: Mutex<Vec<(SessionId, CommandId, By, Command)>>,
    opened: Mutex<Vec<SessionId>>,
    /// 回报先拒几次，原因是对不上任务：父会话还没记下派它的那次调用。
    refusals: Mutex<usize>,
}

impl Table {
    fn sent(&self) -> Vec<(SessionId, CommandId, By, Command)> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn opened(&self) -> Vec<SessionId> {
        self.opened
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl SessionPort for Table {
    fn create(&self, _child: Child) -> Pending<'_, Result<SessionId, String>> {
        Box::pin(async { Ok(session(CHILD)) })
    }

    /// 交回报用不到停和看（施工 7-4）。
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

    fn open(&self, session: SessionId) -> Pending<'_, Result<(), String>> {
        self.opened
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(session);
        Box::pin(async { Ok(()) })
    }

    /// 这份假的会话表用不到读别的会话的日志（施工 C-4）。
    fn read_log(&self, _session: SessionId) -> Pending<'_, Result<miyu_tool::Log, String>> {
        Box::pin(async { Err("no logs here".to_string()) })
    }

    /// 这份假的会话表没有一次性会话（施工 C-5）：用不到的时候一律照送到了算。
    fn held(&self, _session: SessionId) -> Pending<'_, bool> {
        Box::pin(async { false })
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
        let report = matches!(command, Command::Report(_));
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((session, id, by, command));
        let mut refusals = self.refusals.lock().unwrap_or_else(PoisonError::into_inner);
        let outcome = match report && *refusals > 0 {
            true => {
                *refusals -= 1;
                Outcome::Rejected {
                    reason: Reason::UnknownJob,
                    about: Vec::new(),
                }
            }
            false => Outcome::Accepted {
                events: vec![Seq::new(9).expect("从 1 数起")],
            },
        };
        Box::pin(async move { Ok(outcome) })
    }
}

fn session(text: &str) -> SessionId {
    SessionId::parse(text).expect("合写法")
}

/// 等假的会话表收到第 `n` 个命令、叫起第 `n` 个会话。
async fn until(what: &str, done: impl Fn() -> bool) {
    within(what, async {
        while !done() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
}

/// 子会话 `<父会话>/j3` 答完交代以后的回报。
fn expected_report(child: &SessionId) -> (SessionId, CommandId, By, Command) {
    (
        session(PARENT),
        CommandId::parse(&format!("{child}/report/3")).expect("合写法"),
        By::Session(Session { id: child.clone() }),
        Command::Report(ChildReported {
            job: JobId::new(3).expect("从 1 数起"),
            session: child.clone(),
            reason: ChildReason::Done,
            text: "查完了。".to_string(),
            truncated: false,
            person: false,
            by_model: false,
        }),
    )
}

/// 造一个父会话派出来的子会话（任务 `j3`），交代由父会话送进去，替身答「查完了。」。
async fn child_answers(home: &Home, table: &Arc<Table>) -> Handle {
    let parent = session(PARENT);
    let lines = Lines {
        lineage: Some(Lineage {
            parent: parent.clone(),
            depth: 1,
        }),
        sessions: Some(Arc::clone(table) as Arc<dyn SessionPort>),
        command: Some(CommandId::parse(&format!("{PARENT}/j3")).expect("合写法")),
        ..Lines::default()
    };
    let script = Script::new([Play::Says("查完了。")]);
    let handle = home
        .create_full(&script, &Catalog::default(), Opening::default(), lines)
        .await;
    let from_parent = By::Session(Session { id: parent });
    within(
        "交代的回应",
        handle.command(id("p1"), from_parent, say("去查一下")),
    )
    .await
    .expect("会话在跑");
    handle
}

#[tokio::test]
async fn a_report_refused_as_an_unknown_job_is_handed_again_until_it_fits() {
    let home = Home::new();
    let table = Arc::new(Table {
        refusals: Mutex::new(2),
        ..Table::default()
    });
    let handle = child_answers(&home, &table).await;
    until("交了三次", || table.sent().len() == 3).await;
    let expected = expected_report(handle.id());
    assert_eq!(
        table.sent(),
        [expected.clone(), expected.clone(), expected],
        "同一份、同一个命令编号，对上了就不再交"
    );
}

#[tokio::test]
async fn a_child_hands_its_answer_to_its_parent_and_again_when_loaded() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let handle = child_answers(&home, &table).await;
    let child = handle.id().clone();
    until("回报", || table.sent().len() == 1).await;
    let expected = expected_report(&child);
    assert_eq!(table.sent(), std::slice::from_ref(&expected));

    // 停了再载入：最后报的那一份再交一次，同一个命令编号，父会话认得出是重的。
    stop(&handle).await;
    let _loaded = home
        .load_full(
            &child,
            &Script::new([]),
            &Catalog::default(),
            &environment().cwd,
            Some(Arc::clone(&table) as Arc<dyn SessionPort>),
        )
        .await;
    until("再交一次", || table.sent().len() == 2).await;
    assert_eq!(table.sent(), [expected.clone(), expected]);
    assert!(table.opened().is_empty(), "它没派过子代理");
}

#[tokio::test]
async fn a_loaded_parent_wakes_its_unreported_children() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let tools = Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出"))
        .expect("合写法");
    let args = serde_json::json!({"description": "查", "prompt": "去查"}).to_string();
    let script = Script::new([
        Play::calls(&[("subagent", &args)]),
        Play::Says("派出去了。"),
    ]);
    let lines = Lines {
        sessions: Some(Arc::clone(&table) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    let handle = home
        .create_full(&script, &tools, Opening::default(), lines)
        .await;
    let parent = handle.id().clone();
    ask(&handle, "c1", say("派一个去查"))
        .await
        .expect("会话在跑");
    until_logged(&home, &parent, |log| {
        log.iter()
            .any(|event| matches!(event.body, miyu_kernel::event::Body::TurnEnded(_)))
    })
    .await;
    stop(&handle).await;
    let _loaded = home
        .load_full(
            &parent,
            &Script::new([]),
            &tools,
            &environment().cwd,
            Some(Arc::clone(&table) as Arc<dyn SessionPort>),
        )
        .await;
    until("叫起子会话", || table.opened().len() == 1).await;
    assert_eq!(table.opened(), [session(CHILD)]);
    // 交代照常：派的时候送过一次，载入不再送。
    let prompts = table
        .sent()
        .into_iter()
        .filter(|(_, _, _, command)| {
            matches!(command, Command::Send { blocks, .. }
                if blocks == &[Block::Text(Text { text: "去查".to_string() })])
        })
        .count();
    assert_eq!(prompts, 1);
}

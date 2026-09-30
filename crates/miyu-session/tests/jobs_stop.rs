//! 查和停，执行器这一头（施工 7-4，`docs/blueprint/agents.md` 第五条、`session/tools.md`「查和停」）：真的 `jobs` 在会话里列出、
//! 读、停；后台命令用假的（`miyu_tool::testkit::Held`，三个平台一样），子代理的会话表换成假的（停、看它都记下，回报照样送回
//! 父会话）。她停的只记下、不叫醒她，人停的叫醒；停和自己退出撞在一起只认先到的；父会话停下时连它派的一起停，都不叫醒。

mod support;

use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use miyu_kernel::event::{Body, ChildReported, Event, JobReason, JobReported, Level, Permission};
use miyu_kernel::id::{CommandId, JobId, SessionId};
use miyu_kernel::origin::{By, Tool as ByTool};
use miyu_kernel::session::{Command, Outcome};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Child, Handle, Peek, Pending, SessionPort};
use miyu_tool::testkit::{Act, Fake, Held};
use miyu_tool::{Catalog, Exit, JobError, Tool};

use support::*;

/// 子代理的那几条：停掉它、人停它、父会话停下时连它派的一起停。
#[path = "jobs_stop/agents.rs"]
mod agents;

/// 撤销停掉那一轮派出去的（施工 7-8）。
#[path = "jobs_stop/undo.rs"]
mod undo;

/// 子会话的编号。
const CHILD: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

/// 假的会话表：造的子会话是 [`CHILD`]；停、看都记下，看它交回「这一轮说到一半」；发给父会话的命令（停掉的回报）照样交给父会话。
#[derive(Default)]
struct Table {
    parent: OnceLock<Handle>,
    stopped: Mutex<Vec<(SessionId, CommandId, By)>>,
    /// 看它时，它这一轮还没说过话（最近的回答是上一轮的）。
    quiet: bool,
    /// 看它时，它最近的回答有这么多个字（没有的是一句短话）。
    long: Option<usize>,
}

impl Table {
    fn stopped(&self) -> Vec<(SessionId, CommandId, By)> {
        self.stopped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl SessionPort for Table {
    fn create(&self, _child: Child) -> Pending<'_, Result<SessionId, String>> {
        Box::pin(async { Ok(SessionId::parse(CHILD).expect("合写法")) })
    }

    fn command(
        &self,
        session: SessionId,
        id: CommandId,
        by: By,
        command: Command,
    ) -> Pending<'_, Result<Outcome, String>> {
        Box::pin(async move {
            match self.parent.get() {
                Some(parent) if parent.id() == &session => parent
                    .command(id, by, command)
                    .await
                    .map_err(|stopped| stopped.to_string()),
                _ => Ok(Outcome::Accepted { events: Vec::new() }),
            }
        })
    }

    fn open(&self, _session: SessionId) -> Pending<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    /// 这几份假的会话表用不到读别的会话的日志（施工 C-4）。
    fn read_log(&self, _session: SessionId) -> Pending<'_, Result<miyu_tool::Log, String>> {
        Box::pin(async { Err("no logs here".to_string()) })
    }

    fn stop(&self, session: SessionId, id: CommandId, by: By) -> Pending<'_, Result<(), String>> {
        self.stopped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((session, id, by));
        Box::pin(async { Ok(()) })
    }

    fn peek(&self, _session: SessionId) -> Pending<'_, Result<Peek, String>> {
        let this_turn = !self.quiet;
        let reply = self
            .long
            .map_or_else(|| "Half of the tests read.".to_string(), |n| "字".repeat(n));
        Box::pin(async move {
            Ok(Peek {
                reply: Some(reply),
                this_turn,
                working: true,
                doing: vec!["read".to_string()],
            })
        })
    }

    fn sessions(
        &self,
        _owner: miyu_kernel::id::AccountId,
        _stop: miyu_tool::Stop,
    ) -> Pending<'_, Result<Vec<miyu_tool::MainSession>, String>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

/// 真的 `jobs`、`subagent`，和一件假工具 `start`：把 `held` 交给任务端口。
fn tools(held: &Arc<Held>) -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let mut tools: Vec<Arc<dyn Tool>> = miyu_basesystem::tools(&resources)
        .expect("出厂的资源读得出来")
        .into_iter()
        .filter(|tool| ["jobs", "subagent"].contains(&tool.spec().name.as_str()))
        .collect();
    tools.push(Fake::new(
        "start",
        Access::Read,
        Act::Background(Arc::clone(held)),
    ));
    Catalog::new(tools).expect("合写法")
}

/// 完全放开这一级：工具不问。
fn opening() -> Opening {
    Opening {
        permission: Permission {
            level: Level::Full,
            read_only: false,
        },
        ..Opening::default()
    }
}

/// 造一个会话，会话表是 `table`：`start` 起 `held`。
async fn session(home: &Home, script: &Script, held: &Arc<Held>, table: &Arc<Table>) -> Handle {
    let lines = Lines {
        sessions: Some(Arc::clone(table) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    let handle = home
        .create_full(script, &tools(held), opening(), lines)
        .await;
    assert!(table.parent.set(handle.clone()).is_ok());
    handle
}

/// 说一句，等这一轮结束。
async fn talk(handle: &Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// `jobs` 的参数原文。
fn jobs(action: &str, id: Option<&str>) -> String {
    match id {
        Some(id) => serde_json::json!({"action": action, "id": id}).to_string(),
        None => serde_json::json!({"action": action}).to_string(),
    }
}

/// 日志里第 `n` 条工具结果的那一段字。
fn result_text(log: &[Event], n: usize) -> String {
    let results: Vec<String> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(format!("{:?}", result.blocks)),
            _ => None,
        })
        .collect();
    results[n].clone()
}

fn job_reported(log: &[Event]) -> Vec<(&Event, &JobReported)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::JobReported(reported) => Some((event, reported)),
            _ => None,
        })
        .collect()
}

fn child_reported(log: &[Event]) -> Vec<(&Event, &ChildReported)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ChildReported(reported) => Some((event, reported)),
            _ => None,
        })
        .collect()
}

fn turns(log: &[Event]) -> usize {
    log.iter()
        .filter(|event| matches!(event.body, Body::TurnStarted(_)))
        .count()
}

fn j(n: u64) -> JobId {
    JobId::new(n).expect("从 1 数起")
}

#[tokio::test]
async fn she_lists_reads_and_stops_a_command_without_being_woken() {
    let home = Home::new();
    let held = Held::new(&["building\n", "half\n"]);
    let (output, stop, list) = (
        jobs("output", Some("j1")),
        jobs("stop", Some("j1")),
        jobs("list", None),
    );
    let script = Script::new([
        Play::calls(&[("start", "{}")]),
        Play::Says("放出去了。"),
        Play::calls(&[("jobs", output.as_str())]),
        Play::calls(&[("jobs", stop.as_str())]),
        Play::calls(&[("jobs", list.as_str())]),
        Play::Says("停了。"),
    ]);
    let table = Arc::new(Table::default());
    let handle = session(&home, &script, &held, &table).await;
    talk(&handle, "cmd-1", "后台跑").await;
    talk(&handle, "cmd-2", "看一眼再停掉").await;
    let log = home.log(handle.id());
    assert_eq!(
        result_text(&log, 1),
        format!(
            "{:?}",
            text_blocks("building\nhalf\n(j1 is still running.)\n")
        ),
        "读到这时的输出，说还在跑"
    );
    assert_eq!(
        result_text(&log, 2),
        format!("{:?}", text_blocks("Stopped j1.\n"))
    );
    assert_eq!(held.killed(), 1, "整组杀掉了");
    let [(event, reported)] = job_reported(&log)[..] else {
        panic!("记了一条停掉的：{:?}", kinds(&log))
    };
    assert_eq!(reported.reason, JobReason::Stopped);
    assert!(reported.by_model, "她自己停的");
    assert!(reported.duration_ms.is_some() && reported.chars == Some(14));
    let Body::ToolResult(stop_call) = &log
        .iter()
        .filter(|event| matches!(event.body, Body::ToolResult(_)))
        .nth(2)
        .expect("有停的那一次")
        .body
    else {
        unreachable!()
    };
    assert_eq!(
        event.by,
        By::Tool(ByTool {
            call_id: stop_call.call_id
        }),
        "by 是停它的那次 jobs 调用"
    );
    assert_eq!(event.cause, Some(id("cmd-2")), "cause 是那一轮的");
    assert!(
        result_text(&log, 3).contains("j1 command \\\"fake\\\": stopped, "),
        "列出来是停掉的：{}",
        result_text(&log, 3)
    );
    assert_eq!(turns(&log), 2, "她停的不叫醒她");
    held.end(Exit::Code(0));
    assert_eq!(
        within("停了以后", handle.stop_job(j(1), alice(), id("stop-9")))
            .await
            .expect("会话在跑"),
        Err(JobError::Ended),
        "已经结束了的停不了"
    );
}

#[tokio::test]
async fn a_person_stops_a_command_and_she_is_woken() {
    let home = Home::new();
    let held = Held::new(&["x\n"]);
    let script = Script::new([
        Play::calls(&[("start", "{}")]),
        Play::Says("放出去了。"),
        Play::Says("被停掉了。"),
    ]);
    let table = Arc::new(Table::default());
    let handle = session(&home, &script, &held, &table).await;
    talk(&handle, "cmd-1", "后台跑").await;
    let mut pushes = watch(&handle).await;
    let stopped = within("人停", handle.stop_job(j(1), alice(), id("stop-1")))
        .await
        .expect("会话在跑");
    assert_eq!(stopped, Ok(()));
    // 先见结果，后见回应：回应到的时候，那一条已经落了盘、推过了。
    let Some(Ok(first)) = pushes.try_next() else {
        panic!("回应之前推过了")
    };
    assert!(
        matches!(&*first, miyu_session::Pushed::Events(events) if events.iter().any(|event| matches!(event.body, Body::JobReported(_))))
    );
    let log = until_logged(&home, handle.id(), |log| {
        turns(log) == 2
            && log
                .iter()
                .any(|event| matches!(event.body, Body::TurnEnded(_)) && event.seq.get() > 8)
    })
    .await;
    let [(event, reported)] = job_reported(&log)[..] else {
        panic!("记了一条：{:?}", kinds(&log))
    };
    assert_eq!(
        (reported.reason.clone(), reported.by_model),
        (JobReason::Stopped, false)
    );
    assert_eq!(
        (event.by.clone(), event.cause.clone()),
        (alice(), Some(id("stop-1")))
    );
    assert_eq!(held.killed(), 1);
    assert_eq!(
        handle
            .stop_job(j(1), alice(), id("stop-2"))
            .await
            .expect("在跑"),
        Err(JobError::Ended)
    );
    assert_eq!(
        handle
            .stop_job(j(7), alice(), id("stop-3"))
            .await
            .expect("在跑"),
        Err(JobError::Unknown)
    );
}

#[tokio::test]
async fn stopping_and_ending_on_its_own_only_the_first_counts() {
    let home = Home::new();
    let held = Held::new(&[]);
    let script = Script::new([
        Play::calls(&[("start", "{}")]),
        Play::Says("放出去了。"),
        Play::Says("结束了。"),
    ]);
    let table = Arc::new(Table::default());
    let handle = session(&home, &script, &held, &table).await;
    talk(&handle, "cmd-1", "后台跑").await;
    held.end(Exit::Code(3));
    let log = until_logged(&home, handle.id(), |log| !job_reported(log).is_empty()).await;
    assert_eq!(job_reported(&log)[0].1.reason, JobReason::Exited);
    assert_eq!(
        handle
            .stop_job(j(1), alice(), id("stop-1"))
            .await
            .expect("在跑"),
        Err(JobError::Ended),
        "自己先退出了的，停不了，也不记第二条"
    );
    assert_eq!(held.killed(), 0);
    assert_eq!(job_reported(&home.log(handle.id())).len(), 1);
}

/// 一段字的内容块。
fn text_blocks(text: &str) -> Vec<miyu_kernel::block::Block> {
    vec![miyu_kernel::block::Block::Text(miyu_kernel::block::Text {
        text: text.to_string(),
    })]
}

#[tokio::test]
async fn after_a_reload_the_jobs_are_still_known() {
    let home = Home::new();
    let held = Held::new(&[]);
    let script = Script::new([Play::calls(&[("start", "{}")]), Play::Says("放出去了。")]);
    let table = Arc::new(Table::default());
    let handle = session(&home, &script, &held, &table).await;
    talk(&handle, "cmd-1", "后台跑").await;
    stop(&handle).await;
    // 有计划地停下记了 `restarted`；载入以后照日志记得 j1，停它是已经结束了，不是没有。
    let loaded = home.load_with(handle.id(), &script, &tools(&held)).await;
    assert_eq!(
        loaded
            .stop_job(j(1), alice(), id("stop-1"))
            .await
            .expect("在跑"),
        Err(JobError::Ended)
    );
    assert_eq!(
        loaded
            .stop_job(j(2), alice(), id("stop-2"))
            .await
            .expect("在跑"),
        Err(JobError::Unknown)
    );
}

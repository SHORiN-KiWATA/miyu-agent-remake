//! 派子代理，执行器这一头（施工 7-5，`docs/blueprint/agents.md` 第一条、`session/tools.md`「派子代理」）：会话表的端口换成
//! 假的，看执行器交给它的子会话抄对了父会话的每一样、交代记成父会话发的、一步里调几次派几个；编号接着日志往下数，领了没派成
//! 的不回收；工具面上什么时候有 `subagent`（本机、没到深度上限），子会话的 system 接上场所说明；没有端口的派不了。

mod support;

use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Effect, JobKind, JobStarted, Level, Permission, ToolResult};
use miyu_kernel::id::{CommandId, JobId, Seq, SessionId, VenueId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::request::Request;
use miyu_kernel::session::{Command, Outcome};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Child, Handle, Lineage, Pending, SessionPort};
use miyu_tool::Catalog;

use support::*;

/// 派子代理的那件改名 `subagent`，以前造的会话照旧认 `agent`（施工 7-5 再补）。
#[path = "spawn/renamed.rs"]
mod renamed;

/// 深度上限、子代理自己派的编号：拆出去守 500 行上限（施工 C-4 加 `read_log` 顶到了这个上限）。
#[path = "spawn/depth.rs"]
mod depth;

/// 子会话用哪个模型（施工 8-8）：挡位照父会话这一轮的配置解析，不写的抄父会话的引用。
#[path = "spawn/tier.rs"]
mod tier;

/// 场所说明的原文。
const VENUE: &str = include_str!("../../../resources/core/jobs/subagent-venue.txt");
/// 核心的几行（施工 2-7 补）：权限那一句、本机文件的路径那一句，一行一句。
const LINES: &str = concat!(
    include_str!("../../../resources/core/permission-rule.txt"),
    include_str!("../../../resources/core/local-paths-rule.txt"),
);

/// 假的会话表：记下要它造的子会话、发的命令。造的第 n 个子会话编号末位是 n；`failing` 里的第几次造不成。
#[derive(Default)]
struct Table {
    made: Mutex<Vec<Child>>,
    sent: Mutex<Vec<(SessionId, CommandId, By, Command)>>,
    failing: Vec<usize>,
}

impl Table {
    fn failing(failing: &[usize]) -> Arc<Table> {
        Arc::new(Table {
            failing: failing.to_vec(),
            ..Table::default()
        })
    }

    fn made(&self) -> Vec<Child> {
        self.made
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn sent(&self) -> Vec<(SessionId, CommandId, By, Command)> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl SessionPort for Table {
    fn create(&self, child: Child) -> Pending<'_, Result<SessionId, String>> {
        let mut made = self.made.lock().unwrap_or_else(PoisonError::into_inner);
        made.push(child);
        let n = made.len();
        let answer = match self.failing.contains(&n) {
            true => Err("disk full".to_string()),
            false => Ok(child_id(n)),
        };
        Box::pin(async move { answer })
    }

    fn open(&self, _session: SessionId) -> Pending<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    /// 这几份假的会话表用不到读别的会话的日志（施工 C-4）。
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
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((session, id, by, command));
        let events = vec![Seq::new(2).expect("从 1 数起")];
        Box::pin(async move { Ok(Outcome::Accepted { events }) })
    }

    /// 派子代理用不到停和看（施工 7-4）。
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

/// 真的基础系统：`subagent` 在里面。
fn basesystem(home: &Home) -> Catalog {
    Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出")).expect("合写法")
}

/// 调一次 `subagent`。
fn subagent(title: &str, prompt: &str) -> (&'static str, String) {
    let args = serde_json::json!({"description": title, "prompt": prompt});
    ("subagent", args.to_string())
}

/// 一次回复里调这几次。
fn calls(calls: &[(&'static str, String)]) -> Play {
    let calls: Vec<(&str, &str)> = calls.iter().map(|(n, a)| (*n, a.as_str())).collect();
    Play::calls(&calls)
}

/// 说一句，等到磁盘上这一轮结束。
async fn one_turn(home: &Home, handle: &Handle, turns: usize) -> Vec<miyu_kernel::event::Event> {
    let command = format!("cmd-{turns}");
    ask(handle, &command, say("去查一下"))
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

/// 日志里的工具结果，照先后。
fn results(log: &[miyu_kernel::event::Event]) -> Vec<&ToolResult> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

/// 结果里的那一段字。
fn text(result: &ToolResult) -> &str {
    match result.blocks.as_slice() {
        [Block::Text(Text { text })] => text,
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

/// 父会话：完全放开但开着只读，没人能确认，在 `/w` 干活、加了 `/extra`，会话表是 `table`。
async fn parent(home: &Home, script: &Script, table: &Arc<Table>) -> Handle {
    let opening = Opening {
        permission: Permission {
            level: Level::Full,
            read_only: true,
        },
        attended: false,
        cwd: "/w".to_string(),
        dirs: vec!["/extra".to_string()],
        ..Opening::default()
    };
    let lines = Lines {
        sessions: Some(Arc::clone(table) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    home.create_full(script, &basesystem(home), opening, lines)
        .await
}

#[tokio::test]
async fn the_child_copies_the_parent_and_gets_the_task_from_it() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let prompt = "Read src/lib.rs and list what it exports.\nOnly public items.";
    let script = Script::new([
        calls(&[subagent("查导出", prompt)]),
        Play::Says("派出去了。"),
    ]);
    let handle = parent(&home, &script, &table).await;
    let log = one_turn(&home, &handle, 1).await;
    let parent = handle.id().clone();

    let [child] = table.made().try_into().expect("造了一个子会话");
    assert_eq!(
        child,
        Child {
            lineage: Lineage {
                parent: parent.clone(),
                depth: 1
            },
            command: CommandId::parse(&format!("{parent}/j1")).unwrap(),
            persona: "engineer".to_string(),
            owner: alice_account(),
            venue: VenueId::parse("local").unwrap(),
            permission: Permission {
                level: Level::Full,
                read_only: true
            },
            attended: false,
            cwd: "/w".to_string(),
            dirs: vec!["/extra".to_string()],
            model: None,
        }
    );
    // 交代原样、作为父会话发来的话送进子会话，开它的第一轮。
    let [(session, id, by, command)] = table.sent().try_into().expect("送了一次交代");
    assert_eq!(session, child_id(1));
    assert_eq!(id.as_str(), format!("{parent}/j1/prompt"));
    assert_eq!(by, By::Session(Session { id: parent }));
    assert_eq!(command, say(prompt));

    let [result] = results(&log).try_into().expect("一次调用");
    assert_eq!(text(result), "Started subagent j1: \"查导出\".\n");
    assert_eq!(
        result.effects,
        [Effect::JobStarted(JobStarted {
            job: JobId::new(1).unwrap(),
            what: JobKind::Agent,
            title: "查导出".to_string(),
            session: Some(child_id(1)),
        })]
    );
}

#[tokio::test]
async fn several_calls_in_one_step_start_several_children() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        calls(&[subagent("甲", "Task A."), subagent("乙", "Task B.")]),
        Play::Says("两个都派出去了。"),
    ]);
    let handle = parent(&home, &script, &table).await;
    let log = one_turn(&home, &handle, 1).await;

    let mut commands: Vec<String> = table
        .made()
        .iter()
        .map(|child| child.command.as_str().to_string())
        .collect();
    commands.sort();
    let parent = handle.id();
    assert_eq!(commands, [format!("{parent}/j1"), format!("{parent}/j2")]);
    let mut started: Vec<(String, String)> = results(&log)
        .iter()
        .flat_map(|result| &result.effects)
        .map(|effect| match effect {
            Effect::JobStarted(started) => (started.job.to_string(), started.title.clone()),
            other => panic!("只有 job.started：{other:?}"),
        })
        .collect();
    started.sort();
    let jobs: Vec<&str> = started.iter().map(|(job, _)| job.as_str()).collect();
    assert_eq!(jobs, ["j1", "j2"], "一起派的各领各的编号");
    assert_eq!(table.sent().len(), 2);
}

#[tokio::test]
async fn numbers_go_on_after_a_failure_and_a_reload() {
    let home = Home::new();
    let table = Table::failing(&[1]);
    let script = Script::new([
        calls(&[subagent("甲", "Task A.")]),
        Play::Says("派不出去。"),
        calls(&[subagent("乙", "Task B.")]),
        Play::Says("派出去了。"),
        calls(&[subagent("丙", "Task C.")]),
        Play::Says("又派了一个。"),
    ]);
    let handle = parent(&home, &script, &table).await;
    let log = one_turn(&home, &handle, 1).await;
    let [failed] = results(&log).try_into().expect("一次调用");
    assert_eq!(text(failed), "The subagent could not be started.\n");
    assert!(failed.effects.is_empty());
    assert!(table.sent().is_empty(), "没造成的不送交代");

    let log = one_turn(&home, &handle, 2).await;
    assert_eq!(
        text(results(&log)[1]),
        "Started subagent j2: \"乙\".\n",
        "领了没派成的号不回收"
    );

    let id = handle.id().clone();
    stop(&handle).await;
    let port = Some(Arc::clone(&table) as Arc<dyn SessionPort>);
    let tools = basesystem(&home);
    let loaded = home.load_full(&id, &script, &tools, "/w", port).await;
    let log = one_turn(&home, &loaded, 3).await;
    assert_eq!(
        text(results(&log)[2]),
        "Started subagent j3: \"丙\".\n",
        "载入以后接着日志往下数"
    );
    let last = table.made().pop().expect("造过");
    assert!(last.permission.read_only, "载入以后照日志里的权限");
    assert!(!last.attended, "载入以后照快照里的能不能确认");
    assert_eq!(last.lineage.depth, 1);
}

#[tokio::test]
async fn without_the_table_the_agent_is_not_started() {
    let home = Home::new();
    let script = Script::new([calls(&[subagent("甲", "Task A.")]), Play::Says("派不了。")]);
    let handle = home
        .create_as(&script, &basesystem(&home), Opening::default())
        .await;
    let log = one_turn(&home, &handle, 1).await;
    let [result] = results(&log).try_into().expect("一次调用");
    assert_eq!(text(result), "The subagent could not be started.\n");
}

/// 造一个会话、说一句：交回它发出的第一次请求，和这一轮里调 `subagent` 的结果（`spawn/depth.rs`、`spawn/renamed.rs`
/// 都用得到）。
async fn first_request(lines: Lines, table: &Arc<Table>) -> (Request, Vec<ToolResult>) {
    let home = Home::new();
    let script = Script::new([calls(&[subagent("甲", "Task A.")]), Play::Says("好。")]);
    let lines = Lines {
        sessions: Some(Arc::clone(table) as Arc<dyn SessionPort>),
        ..lines
    };
    let handle = home
        .create_full(&script, &basesystem(&home), Opening::default(), lines)
        .await;
    let log = one_turn(&home, &handle, 1).await;
    let results = results(&log).into_iter().cloned().collect();
    (script.requests()[0].1.clone(), results)
}

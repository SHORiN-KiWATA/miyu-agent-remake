//! 列会话，执行器这一头（施工 C-3，`docs/blueprint/cross-session.md` 第一条、第九条）：会话表的端口换成假的，看 `sessions` 经它
//! 要的是这个会话的属主的主会话，交给工具时拿掉她自己；载入的会话照样列。工具面：本机的主会话有 `sessions`，子会话、群里
//! 没有，别的工具一件不少。

mod support;

use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{AccountId, CommandId, SessionId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::request::Request;
use miyu_kernel::session::{Command, Outcome};
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Child, Handle, Lineage, Pending, SessionPort};
use miyu_store::root::DataRoot;
use miyu_tool::{Catalog, Log, MainSession, Stop};

use support::*;

/// 假的会话表：列会话时交回 `listed` 再加上问它的那个会话，记下每次问的属主；读别的会话的日志（施工 C-4）照
/// `root` 上真实的目录读，和生产里一样不载入它。
struct Table {
    root: Option<DataRoot>,
    listed: Vec<MainSession>,
    asked: Mutex<Vec<AccountId>>,
    this: Mutex<Option<SessionId>>,
}

impl Default for Table {
    /// 没给数据根的：读别的会话的日志用不上（列会话不用它）。
    fn default() -> Table {
        Table {
            root: None,
            listed: Vec::new(),
            asked: Mutex::new(Vec::new()),
            this: Mutex::new(None),
        }
    }
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
        owner: AccountId,
        _stop: Stop,
    ) -> Pending<'_, Result<Vec<MainSession>, String>> {
        self.asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(owner);
        let mut listed = self.listed.clone();
        // 会话表交回的含她自己。
        if let Some(this) = self
            .this
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
        {
            listed.push(main(this.as_str(), "我自己", "2026-10-01T09:00:00.000Z"));
        }
        Box::pin(async move { Ok(listed) })
    }

    /// 只算出会话 `session` 的真实目录（施工 C-4），和生产里一样不读盘、不载入它。没给数据根的这份假会话表
    /// 用不到（这几个测试不读别的会话）。
    fn read_log(&self, session: SessionId) -> Pending<'_, Result<Log, String>> {
        let root = self.root.clone();
        Box::pin(async move {
            let root = root.ok_or_else(|| "no data root in this fake".to_string())?;
            let dir = root.session_dir(&alice_account(), &session);
            Ok(Log::new(LogDir(dir)))
        })
    }
}

fn main(id: &str, title: &str, at: &str) -> MainSession {
    MainSession {
        id: SessionId::parse(id).expect("合写法"),
        title: title.to_string(),
        cwd: "~/src".to_string(),
        busy: false,
        last_active: Timestamp::parse(at).expect("合写法"),
    }
}

/// 真的基础系统。
fn basesystem(home: &Home) -> Catalog {
    Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出")).expect("合写法")
}

fn list() -> Play {
    Play::calls(&[("sessions", "{}")])
}

async fn session(home: &Home, script: &Script, table: Option<&Arc<Table>>, lines: Lines) -> Handle {
    let lines = Lines {
        sessions: table.map(|table| Arc::clone(table) as Arc<dyn SessionPort>),
        ..lines
    };
    home.create_full(script, &basesystem(home), Opening::default(), lines)
        .await
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

fn names(request: &Request) -> Vec<&str> {
    request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect()
}

#[tokio::test]
async fn it_asks_for_the_owners_sessions_and_leaves_herself_out() {
    let home = Home::new();
    let table = Arc::new(Table {
        listed: vec![main(
            "0192f3a0-2222-7abc-8def-5566e9f03b21",
            "修 CI",
            "2026-10-01T06:03:00.000Z",
        )],
        ..Table::default()
    });
    let script = Script::new([list(), Play::Says("好。")]);
    let handle = session(&home, &script, Some(&table), Lines::default()).await;
    *table.this.lock().unwrap_or_else(PoisonError::into_inner) = Some(handle.id().clone());
    let log = one_turn(&home, &handle, 1).await;
    let text = last_result(&log);
    let mut lines = text.lines();
    assert_eq!(
        lines.next(),
        Some(format!("You are session {}.", handle.id().short()).as_str())
    );
    let rows: Vec<&str> = lines.collect();
    assert_eq!(rows.len(), 1, "她自己不在下面：{text}");
    assert!(
        rows[0].starts_with("e9f03b21 \"修 CI\" in ~/src: idle"),
        "{text}"
    );
    assert_eq!(
        *table.asked.lock().unwrap_or_else(PoisonError::into_inner),
        [alice_account()],
        "照这个会话的属主要"
    );
}

#[tokio::test]
async fn without_a_table_she_hears_there_are_no_others() {
    let home = Home::new();
    let script = Script::new([list(), Play::Says("好。")]);
    let handle = session(&home, &script, None, Lines::default()).await;
    let log = one_turn(&home, &handle, 1).await;
    assert_eq!(last_result(&log), "You have no other sessions.\n");
}

#[tokio::test]
async fn only_a_local_main_session_has_it() {
    let home = Home::new();
    let all: Vec<String> = basesystem(&home)
        .specs()
        .map(|spec| spec.name.clone())
        .collect();
    let parent = SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d99").expect("合写法");
    let child = Lines {
        lineage: Some(Lineage { parent, depth: 1 }),
        ..Lines::default()
    };
    let group = Lines {
        venue: VenueId::parse("qq:group:123456").expect("合写法"),
        ..Lines::default()
    };
    let without = |dropped: &[&str]| -> Vec<String> {
        all.iter()
            .filter(|name| !dropped.contains(&name.as_str()))
            .cloned()
            .collect()
    };
    for (lines, want) in [
        (Lines::default(), all.clone()),
        (child, without(&["sessions"])),
        (group, without(&["sessions", "subagent", "message_agent"])),
    ] {
        let script = Script::new([Play::Says("好。")]);
        let handle = session(&home, &script, None, lines).await;
        one_turn(&home, &handle, 1).await;
        let requests = script.requests();
        assert_eq!(names(&requests[0].1), want, "别的工具一件不少");
    }
    assert!(all.iter().any(|name| name == "sessions"), "目录里有它");
}

#[tokio::test]
async fn a_child_calling_it_is_told_there_is_no_such_tool() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let parent = SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d99").expect("合写法");
    let child = Lines {
        lineage: Some(Lineage { parent, depth: 1 }),
        ..Lines::default()
    };
    let script = Script::new([list(), Play::Says("好。")]);
    let handle = session(&home, &script, Some(&table), child).await;
    let log = one_turn(&home, &handle, 1).await;
    assert!(
        !last_result(&log).starts_with("You are session"),
        "子会话列不了：{}",
        last_result(&log)
    );
    assert!(
        table
            .asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty(),
        "端口一次都没问"
    );
}

/// 载入的主会话照 `session.created` 知道自己是主会话：载入以后照样列得出来。
#[tokio::test]
async fn a_loaded_main_session_still_lists() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([Play::Says("好。"), list(), Play::Says("好。")]);
    let handle = session(&home, &script, Some(&table), Lines::default()).await;
    one_turn(&home, &handle, 1).await;
    let id = handle.id().clone();
    stop(&handle).await;
    let port = Some(Arc::clone(&table) as Arc<dyn SessionPort>);
    let loaded = home
        .load_full(&id, &script, &basesystem(&home), &environment().cwd, port)
        .await;
    let log = one_turn(&home, &loaded, 2).await;
    assert_eq!(
        last_result(&log),
        format!(
            "You are session {}.\nYou have no other sessions.\n",
            id.short()
        )
    );
    assert_eq!(
        table
            .asked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len(),
        1
    );
}

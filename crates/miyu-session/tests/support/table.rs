//! 假的会话表（施工 C-6 的 `watch.rs` 用起，施工 V-2 再补挪到这里，闲够了退下的测试也用）：订的照 `gone` 回；命令都接受；
//! 记下每一次订、每一个命令。看得到的别的会话只有 [`OTHER`]。

use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome};
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::Script;
use miyu_session::{Child, Handle, NotWatched, Pending, SessionPort};
use miyu_tool::MainSession;

use super::{Home, Lines, Opening};

/// 被等的会话：短编号 `9f03b21c`。
pub const OTHER: &str = "0192f3a0-2222-7abc-8def-55669f03b21c";
/// 另一个在等的会话。
pub const WAITER: &str = "0192f3a0-3333-7abc-8def-0c5d77aa0c5d";
/// 派出去的子代理的子会话。
pub const CHILD: &str = "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91";

/// 一次订：被等的、在等的、起算时刻。
pub type Placed = (SessionId, SessionId, Timestamp);

/// 假的会话表：订的照 `gone` 回；命令都接受；记下每一次订、每一个命令。看得到的别的会话只有 `OTHER`。
#[derive(Default)]
pub struct Table {
    pub gone: bool,
    pub placed: Mutex<Vec<Placed>>,
    pub commands: Mutex<Vec<(SessionId, CommandId, By, Command)>>,
}

impl Table {
    pub fn placed(&self) -> Vec<Placed> {
        self.placed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 送出去的通知：发给谁、编号、谁发的、带的那一行。
    pub fn notices(&self) -> Vec<(SessionId, CommandId, By, Option<String>)> {
        self.commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .filter_map(|(to, id, by, command)| match command {
                Command::PeerIdle { status } => {
                    Some((to.clone(), id.clone(), by.clone(), status.clone()))
                }
                _ => None,
            })
            .collect()
    }
}

impl SessionPort for Table {
    fn create(&self, _child: Child) -> Pending<'_, Result<SessionId, String>> {
        Box::pin(async { Ok(sid(CHILD)) })
    }

    fn open(&self, _session: SessionId) -> Pending<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    fn command(
        &self,
        session: SessionId,
        id: CommandId,
        by: By,
        command: Command,
    ) -> Pending<'_, Result<Outcome, String>> {
        self.commands
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((session, id, by, command));
        Box::pin(async { Ok(Outcome::Accepted { events: Vec::new() }) })
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
        _stop: miyu_tool::Stop,
    ) -> Pending<'_, Result<Vec<MainSession>, String>> {
        let other = MainSession {
            id: sid(OTHER),
            title: String::new(),
            cwd: "~/src".to_string(),
            busy: true,
            last_active: Timestamp::parse("2026-10-01T09:00:00.000Z").expect("合写法"),
        };
        Box::pin(async move { Ok(vec![other]) })
    }

    fn read_log(&self, _session: SessionId) -> Pending<'_, Result<miyu_tool::Log, String>> {
        Box::pin(async { Err("no logs here".to_string()) })
    }

    fn held(&self, _session: SessionId) -> Pending<'_, bool> {
        Box::pin(async { false })
    }

    fn watch(
        &self,
        session: SessionId,
        watcher: SessionId,
        since: Timestamp,
    ) -> Pending<'_, Result<(), NotWatched>> {
        self.placed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((session, watcher, since));
        let gone = self.gone;
        Box::pin(async move {
            match gone {
                true => Err(NotWatched::Gone),
                false => Ok(()),
            }
        })
    }
}

pub fn sid(text: &str) -> SessionId {
    SessionId::parse(text).expect("合写法")
}

/// 真的基础系统：`send_message`、`subagent` 在里面。
pub fn basesystem(home: &Home) -> miyu_tool::Catalog {
    miyu_tool::Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出"))
        .expect("合写法")
}

/// 造一个主会话，会话表是 `table`。
pub async fn session(home: &Home, script: &Script, table: &Arc<Table>) -> Handle {
    let lines = Lines {
        sessions: Some(Arc::clone(table) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    home.create_full(script, &basesystem(home), Opening::default(), lines)
        .await
}

/// 载入会话 `id`，会话表是 `table`。
pub async fn reload(home: &Home, id: &SessionId, script: &Script, table: &Arc<Table>) -> Handle {
    let port = Arc::clone(table) as Arc<dyn SessionPort>;
    home.load_full(id, script, &basesystem(home), "~/src/miyu", Some(port))
        .await
}

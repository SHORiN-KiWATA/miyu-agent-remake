//! 会话表交给会话的端口（施工 7-5，`docs/blueprint/agents.md`「在哪」）：会话 actor 比会话表低一层，派子代理要造子会话、
//! 给子会话发命令，经 `miyu-session` 定义的 [`SessionPort`]，这里照会话表实现。会话表造会话、载入时交进去一份。子会话向上
//! 回报、父会话载入以后叫起子会话也经它（施工 7-6）。
//!
//! 端口拿着核心的弱引用：会话由核心的会话表拿着，端口再强拿着核心就成了环。核心没了（正在退出）的，派不了。
//!
//! 停子代理、看它在做什么也经它（施工 7-4）：停下子会话、照它的日志算它这会儿的样子。她列会话也经它（施工 C-3）：和
//! `session.list` 同一个函数算（`crate::list`）。她读别的会话的日志也经它（施工 C-4）：只算出目录，不载入那个会话。
//! 她发给别的会话时，对方是不是没人看着的一次性会话也经它看（施工 C-5）。她订「空了告诉我」也经它：把「谁在等」交给被等的
//! 那个会话的 actor，它没载入的先载入（施工 C-6）。

use std::path::PathBuf;
use std::sync::{Arc, Weak};

use miyu_kernel::event::Event;
use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome, Queued};
use miyu_kernel::time::Timestamp;
use miyu_session::{Child, NotWatched, Peek, Pending, SessionPort, peek};
use miyu_store::index::Row;
use miyu_store::log::{read_events, read_segments};
use miyu_tool::{Log, MainSession, ReadLog, Stop};

use crate::Core;
use crate::list::scan;
use crate::refusal::Refusal;

/// 交给会话的端口：造子会话、给会话发命令都照会话表的规矩。
pub(crate) fn port(core: &Arc<Core>) -> Arc<dyn SessionPort> {
    Arc::new(Table(Arc::downgrade(core)))
}

/// 会话表那一头。
struct Table(Weak<Core>);

impl Table {
    /// 核心还在。
    fn core(&self) -> Result<Arc<Core>, String> {
        self.0
            .upgrade()
            .ok_or_else(|| "the core is shutting down".to_string())
    }
}

impl SessionPort for Table {
    fn listing(&self, session: SessionId) {
        if let Ok(core) = self.core() {
            core.listing.changed(&core, session);
        }
    }

    fn create(&self, child: Child) -> Pending<'_, Result<SessionId, String>> {
        Box::pin(async move {
            let core = self.core()?;
            core.sessions.spawn(&core, child).await
        })
    }

    fn open(&self, session: SessionId) -> Pending<'_, Result<(), String>> {
        Box::pin(async move {
            let core = self.core()?;
            core.sessions
                .get(&core, &session, None, None)
                .await
                .map(|_| ())
                .map_err(|refusal| format!("session {session} not opened: {refusal:?}"))
        })
    }

    fn command(
        &self,
        session: SessionId,
        id: CommandId,
        by: By,
        command: Command,
    ) -> Pending<'_, Result<Outcome, String>> {
        Box::pin(async move {
            let core = self.core()?;
            let found = core
                .sessions
                .get(&core, &session, None, None)
                .await
                .map_err(|refusal| format!("session {session} not found: {refusal:?}"))?;
            match found.handle.command(id, by, command).await {
                Ok(outcome) => Ok(outcome),
                Err(_) => {
                    core.sessions.forget(&session).await;
                    Err(format!("session {session} stopped"))
                }
            }
        })
    }

    /// 停下子会话（施工 7-4）：先打断它在跑的一轮（排着的退回，没有在跑的不要紧），再停掉它派出去的，连它们派的。
    fn stop(&self, session: SessionId, id: CommandId, by: By) -> Pending<'_, Result<(), String>> {
        Box::pin(async move {
            let core = self.core()?;
            let found = core
                .sessions
                .get(&core, &session, None, None)
                .await
                .map_err(|refusal| format!("session {session} not found: {refusal:?}"))?;
            let interrupt = Command::Interrupt {
                queued: Queued::Return,
            };
            let stopped = match found
                .handle
                .command(id.clone(), by.clone(), interrupt)
                .await
            {
                Ok(_) => found.handle.stop_jobs(by, id).await,
                Err(stopped) => Err(stopped),
            };
            if stopped.is_err() {
                core.sessions.forget(&session).await;
                return Err(format!("session {session} stopped"));
            }
            Ok(())
        })
    }

    /// 子会话这会儿的样子（施工 7-4）：在阻塞线程里只读地读它的日志，不载入它。
    fn peek(&self, session: SessionId) -> Pending<'_, Result<Peek, String>> {
        Box::pin(async move {
            let core = self.core()?;
            let dir = core.root.session_dir(&core.admin, &session);
            let events = tokio::task::spawn_blocking(move || read_events(&dir))
                .await
                .map_err(|error| error.to_string())?
                .map_err(|error| error.to_string())?;
            Ok(peek(&events))
        })
    }

    /// 属主是 `owner` 的主会话（施工 C-3）：会话表里这时忙着的先记下，再在阻塞线程里照 `session.list` 的办法读。
    fn sessions(
        &self,
        owner: AccountId,
        stop: Stop,
    ) -> Pending<'_, Result<Vec<MainSession>, String>> {
        Box::pin(async move {
            let core = self.core()?;
            let busy = core.sessions.busy_ids().await;
            let root = core.root.clone();
            let index = core.index_for(&owner);
            // 场所会话不列（施工 O-3）：群里、QQ 私聊里的会话不给本机的她看，`history` 也读不到。
            let main = |row: &Row| row.parent.is_none() && crate::list::local(row);
            let listed = tokio::task::spawn_blocking(move || {
                scan(&root, &owner, index.as_deref(), &busy, main, None, &stop)
            })
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| format!("sessions not listed: {error}"))?;
            Ok(listed
                .into_iter()
                .map(|listed| MainSession {
                    id: listed.id,
                    title: listed.title,
                    cwd: listed.cwd,
                    busy: listed.busy,
                    last_active: listed.last_active,
                })
                .collect())
        })
    }

    /// 只读地开会话 `session` 的日志（施工 C-4）：只算出它的目录，不读盘。核心正在停的才出错；日志坏了、读不了的要等
    /// 交回的 [`Log`] 读的时候才知道。
    fn read_log(&self, session: SessionId) -> Pending<'_, Result<Log, String>> {
        Box::pin(async move {
            let core = self.core()?;
            let dir = core.root.session_dir(&core.admin, &session);
            Ok(Log::new(Dir(dir)))
        })
    }

    /// 会话 `session` 这时是不是没人看着的一次性会话（施工 C-5）：`send_message` 刚经 [`Table::command`] 把它载入过，
    /// 这里照会话表里的 `Handle` 看（`cross-session.md` 第三条第 4 款）；核心正在停、这个会话不在表里的，当不是。
    fn held(&self, session: SessionId) -> Pending<'_, bool> {
        Box::pin(async move {
            let Ok(core) = self.core() else {
                return false;
            };
            match core.sessions.get(&core, &session, None, None).await {
                Ok(found) => found.handle.oneshot() && !found.handle.watched(),
                Err(_) => false,
            }
        })
    }

    fn watch(
        &self,
        session: SessionId,
        watcher: SessionId,
        since: Timestamp,
    ) -> Pending<'_, Result<(), NotWatched>> {
        Box::pin(self.watch_session(session, watcher, since))
    }
}

impl Table {
    /// 「空了告诉我」（施工 C-6，`cross-session.md` 第六条第 3 款）：照会话表的规矩找到 `session`（没在跑的先载入），把
    /// 「`watcher` 在等」交给它的 actor。找不到的（删了、从来没有）是不在了；载入不了、它停了、核心正在停的交回原因。
    async fn watch_session(
        &self,
        session: SessionId,
        watcher: SessionId,
        since: Timestamp,
    ) -> Result<(), NotWatched> {
        let core = self.core().map_err(NotWatched::Failed)?;
        let found = match core.sessions.get(&core, &session, None, None).await {
            Ok(found) => found,
            Err(refusal) if refusal == Refusal::NOT_FOUND => return Err(NotWatched::Gone),
            Err(refusal) => {
                return Err(NotWatched::Failed(format!(
                    "session {session} not opened: {refusal:?}"
                )));
            }
        };
        if found.handle.watch(watcher, since).is_err() {
            core.sessions.forget(&session).await;
            return Err(NotWatched::Failed(format!("session {session} stopped")));
        }
        Ok(())
    }
}

/// 一个会话目录的只读入口，给别的会话读用（施工 C-4）：和这个会话自己那份（`miyu-session` 里的 `LogDir`）是同一个
/// 读法（[`read_segments`]），照会话表这一层直接依赖 `miyu-store` 的先例（[`read_events`] 已经这样用），不必把
/// `miyu-session` 里那份公开出来。
struct Dir(PathBuf);

impl ReadLog for Dir {
    fn read(&self, each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        read_segments(&self.0, each).map_err(|error| error.to_string())
    }
}

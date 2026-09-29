//! 会话表交给会话的端口（施工 7-5，`docs/blueprint/agents.md`「在哪」）：会话 actor 比会话表低一层，派子代理要造子会话、
//! 给子会话发命令，经 `miyu-session` 定义的 [`SessionPort`]，这里照会话表实现。会话表造会话、载入时交进去一份。子会话向上
//! 回报、父会话载入以后叫起子会话也经它（施工 7-6）。
//!
//! 端口拿着核心的弱引用：会话由核心的会话表拿着，端口再强拿着核心就成了环。核心没了（正在退出）的，派不了。
//!
//! 停子代理、看它在做什么也经它（施工 7-4）：停下子会话、照它的日志算它这会儿的样子。

use std::sync::{Arc, Weak};

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome, Queued};
use miyu_session::{Child, Peek, Pending, SessionPort, peek};
use miyu_store::log::read_events;

use crate::Core;

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
}

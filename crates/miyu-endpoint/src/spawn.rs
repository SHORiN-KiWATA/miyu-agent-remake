//! 会话表交给会话的端口（施工 7-5，`docs/blueprint/agents.md`「在哪」）：会话 actor 比会话表低一层，派子代理要造子会话、
//! 给子会话发命令，经 `miyu-session` 定义的 [`SessionPort`]，这里照会话表实现。会话表造会话、载入时交进去一份。
//!
//! 端口拿着核心的弱引用：会话由核心的会话表拿着，端口再强拿着核心就成了环。核心没了（正在退出）的，派不了。

use std::sync::{Arc, Weak};

use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome};
use miyu_session::{Child, Pending, SessionPort};

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
}

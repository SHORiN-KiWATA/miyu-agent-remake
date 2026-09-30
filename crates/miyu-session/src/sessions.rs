//! 列会话，执行器这一头（`docs/blueprint/cross-session.md` 第一条，`session/tools.md`「列会话」，施工 C-3）：经会话表的端口
//! （[`SessionPort::sessions`]）要这个会话的属主的主会话，拿掉它自己，交给 `sessions`。
//!
//! 只有本机的主会话有这个端口（[`Agents::lists_sessions`]），和工具面上有没有 `sessions` 是同一个判断。
//!
//! [`SessionPort::sessions`]: crate::spawn::SessionPort::sessions

use std::sync::Arc;

use miyu_kernel::id::SessionId;
use miyu_tool::{Listing, SessionsPort, Stop};

use crate::TARGET;
use crate::agents::Agents;

/// 交给一次调用的列会话端口：这个会话能列的才有。
pub(crate) fn for_call(agents: &Arc<Agents>) -> Option<Arc<dyn SessionsPort>> {
    Agents::lists_sessions(&agents.venue, agents.parent.as_ref()).then(|| {
        Arc::new(Lister {
            agents: Arc::clone(agents),
        }) as Arc<dyn SessionsPort>
    })
}

/// 一次调用的列会话端口。
struct Lister {
    agents: Arc<Agents>,
}

impl SessionsPort for Lister {
    fn this(&self) -> &SessionId {
        &self.agents.session
    }

    fn list<'a>(&'a self, stop: &'a Stop) -> Listing<'a> {
        Box::pin(async move {
            let agents = &self.agents;
            let mut listed = agents
                .port
                .sessions(agents.owner.clone(), stop.clone())
                .await
                .inspect_err(|error| {
                    tracing::warn!(target: TARGET, error = error.as_str(), "sessions not listed");
                })?;
            listed.retain(|session| session.id != agents.session);
            Ok(listed)
        })
    }
}

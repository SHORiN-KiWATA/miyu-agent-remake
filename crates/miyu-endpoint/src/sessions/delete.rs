//! 删会话（施工 3-8 三补，`docs/blueprint/protocol.md` 的 `session.delete`，`agents.md` 第七条第 5、6 条）。
//!
//! 删的是一个子会话、它的父会话还在的，先照人停掉它（`job.stop` 那条路，施工 7-4）：打断它这一轮、停掉它派的，父会话记一条
//! `child.reported`（`stopped`，不带 `by_model`，叫醒父会话）。这一步经会话表的端口来回，在拿表的锁之前做。然后会话表拿着
//! 锁办完一整件，这期间谁都载入不了、造不了子会话：
//!
//! 1. 先从磁盘上认出它派出去的子会话，一层层往下（`session.created` 的 `parent`）；
//! 2. 它自己在跑的，交给它的 actor 问删不删得了：有回合在进行、正在改回文件的拒绝，什么都不动；没在跑的不载入：载入会
//!    收尾崩了的回合、接着干被重启打断的回合、叫起子会话，删之前都不该做；
//! 3. 在跑的子会话一层层停下，不管它们忙不忙；它们和它自己的后台命令整组杀掉、不记回报；
//! 4. 目录挪进回收处，从最深的子会话起，它自己最后：半路崩了，它还在原处、列得出来，再删一次接着挪完。

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use miyu_kernel::event::Body;
use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_session::job_in;
use miyu_store::log::{OpenError, first_event};
use miyu_store::root::DataRoot;
use miyu_store::trash;

use super::{Sessions, admin, now};
use crate::Core;
use crate::refusal::Refusal;

impl Sessions {
    /// 删会话 `id`：子会话先照人停掉它，再照上面四步。`request` 是删它的那条命令的编号。回应 `{}` 之前目录都挪好了。
    pub(crate) async fn delete(
        &self,
        core: &Arc<Core>,
        id: &SessionId,
        request: CommandId,
    ) -> Result<(), Refusal> {
        self.stop_as_job(core, id, request).await;
        let mut open = self.open.lock().await;
        let running = open.running.get(id).map(|running| running.handle.clone());
        let family = family(core, id, running.is_none()).await?;
        if let Some(handle) = running {
            match handle.delete().await {
                Ok(Ok(())) => {}
                Ok(Err(reason)) => return Err(Refusal::kernel(reason)),
                // 自己停了的（写不进去、出了 bug）：它的后台命令随它停的时候杀掉了，照样挪。
                Err(_) => {
                    tracing::debug!(target: "miyu::endpoint", session = id.as_str(), "already stopped")
                }
            }
            open.running.remove(id);
        }
        for child in &family {
            if let Some(running) = open.running.remove(child)
                && running.handle.discard().await.is_err()
            {
                tracing::debug!(target: "miyu::endpoint", session = child.as_str(), "already stopped");
            }
        }
        // 重发的造会话不再交回删了的会话。
        open.created
            .retain(|(_, session)| session != id && !family.contains(session));
        let (root, account, at) = (core.root.clone(), core.admin.clone(), now());
        let order: Vec<SessionId> = family.into_iter().rev().chain([id.clone()]).collect();
        let moved = tokio::task::spawn_blocking(move || {
            order.into_iter().try_for_each(|session| {
                trash::discard(&root, &account, &session, at).map_err(|error| (session, error))
            })
        })
        .await;
        match moved {
            Ok(Ok(())) => Ok(()),
            Ok(Err((session, error))) => {
                tracing::warn!(target: "miyu::endpoint", session = session.as_str(), error = %error, "session not deleted");
                Err(Refusal::INTERNAL)
            }
            Err(error) => {
                tracing::error!(target: "miyu::endpoint", error = %error, "delete panicked");
                Err(Refusal::INTERNAL)
            }
        }
    }
}

impl Sessions {
    /// 会话 `id` 是一个子会话、父会话还在的：照人停掉它（`job.stop` 那条路），父会话收到 `stopped` 的回报、被叫醒
    /// （2026-09-30 主会话定：删一个子会话等于人先停掉它再删，父会话不会白等）。不是子会话的、造它的命令编号读不出任务编号
    /// 的、父会话已经不在（删了）或者载入不了的，什么都不做：删照样往下走。它已经报过、已经被停过的，父会话说没有这个任务，
    /// 也不要紧。
    async fn stop_as_job(&self, core: &Arc<Core>, id: &SessionId, request: CommandId) {
        let dir = core.root.session_dir(&core.admin, id);
        let Ok(Ok(first)) = tokio::task::spawn_blocking(move || first_event(&dir)).await else {
            return;
        };
        let (Body::SessionCreated(created), Some(cause)) = (first.body, first.cause) else {
            return;
        };
        let Some(parent) = created.parent else {
            return;
        };
        let Some(job) = job_in(&parent, &cause) else {
            return;
        };
        let Ok(found) = self.get(core, &parent, None, None).await else {
            return;
        };
        match found.handle.stop_job(job, admin(core), request).await {
            Ok(Ok(())) => {
                tracing::debug!(target: "miyu::endpoint", session = id.as_str(), "stopped before deletion");
            }
            // 已经报过、被停过：父会话早知道了。
            Ok(Err(_)) => {}
            Err(_) => self.forget(&parent).await,
        }
    }
}

/// 会话 `id` 派出去的子会话，子、孙一层层往下，照磁盘上各个会话 `session.created` 的 `parent` 认。`check` 是真的，先看
/// 它自己在不在磁盘上：没有日志（第一行还没写完的也算）的是没有这个会话。在阻塞线程里读。
async fn family(core: &Core, id: &SessionId, check: bool) -> Result<Vec<SessionId>, Refusal> {
    let (root, account, id) = (core.root.clone(), core.admin.clone(), id.clone());
    match tokio::task::spawn_blocking(move || descendants(&root, &account, &id, check)).await {
        Ok(found) => found,
        Err(error) => {
            tracing::error!(target: "miyu::endpoint", error = %error, "delete panicked");
            Err(Refusal::INTERNAL)
        }
    }
}

/// 同 [`family`]，在阻塞线程里。第一条读不出来的会话认不出父会话，不算谁的子会话。
fn descendants(
    root: &DataRoot,
    account: &AccountId,
    id: &SessionId,
    check: bool,
) -> Result<Vec<SessionId>, Refusal> {
    if check && let Err(OpenError::Missing(_)) = first_event(&root.session_dir(account, id)) {
        return Err(Refusal::NOT_FOUND);
    }
    let ids = root.sessions(account).map_err(|error| {
        tracing::warn!(target: "miyu::endpoint", error = %error, "sessions not listed");
        Refusal::INTERNAL
    })?;
    let mut children: BTreeMap<SessionId, Vec<SessionId>> = BTreeMap::new();
    for child in ids {
        if let Ok(event) = first_event(&root.session_dir(account, &child))
            && let Body::SessionCreated(created) = event.body
            && let Some(parent) = created.parent
        {
            children.entry(parent).or_default().push(child);
        }
    }
    let mut found = Vec::new();
    let mut next = VecDeque::from([id.clone()]);
    while let Some(parent) = next.pop_front() {
        for child in children.remove(&parent).unwrap_or_default() {
            next.push_back(child.clone());
            found.push(child);
        }
    }
    Ok(found)
}

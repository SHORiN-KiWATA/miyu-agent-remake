//! 删会话（施工 3-8 三补，`docs/blueprint/protocol.md` 的 `session.delete`，`agents.md` 第七条第 5、6 条）。
//!
//! 会话表拿着锁办完一整件，这期间谁都载入不了、造不了子会话、给它发不了命令（施工 7-8：停子会话、给父会话送回报原来在拿
//! 锁之前经端口来回，中间人能再叫醒它）：
//!
//! 1. 先从磁盘上认出它派出去的子会话，一层层往下（`session.created` 的 `parent`）；
//! 2. 它是一个子会话、父会话还在的（找父会话照表的规矩，没在跑的载入），删它等于人先停掉它再删（2026-09-30 主会话定）：
//!    在跑的不问忙不忙就停下；
//! 3. 别的，它自己在跑的，交给它的 actor 问删不删得了：有回合在进行、正在改回文件的拒绝，什么都不动；没在跑的不载入：载入
//!    会收尾崩了的回合、接着干被重启打断的回合、叫起子会话，删之前都不该做；
//! 4. 在跑的子会话一层层停下，不管它们忙不忙；它们和它自己的后台命令整组杀掉、不记回报；
//! 5. 第 2 条那种，父会话照人停它记一条 `child.reported`（`stopped`，不带 `by_model`，叫醒父会话，`Handle::stopped_child`）：
//!    回报在父会话的 actor 里当场记，不经会话表；
//! 6. 目录挪进回收处，从最深的子会话起，它自己最后：半路崩了，它还在原处、列得出来，再删一次接着挪完。挪走一个，删掉它在
//!    会话列表的索引里的那一行（施工 3-8 七补），拿掉回合库里它的（施工 R-2 上）。

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use miyu_kernel::event::Body;
use miyu_kernel::id::{AccountId, JobId, SessionId};
use miyu_session::{Handle, job_in};
use miyu_store::log::{OpenError, first_event};
use miyu_store::root::DataRoot;
use miyu_store::trash;

use super::{Open, Sessions, now};
use crate::Core;
use crate::list::forget;
use crate::refusal::Refusal;

impl Sessions {
    /// 删会话 `id`，照上面六步。回应 `{}` 之前目录都挪好了。
    pub(crate) async fn delete(&self, core: &Arc<Core>, id: &SessionId) -> Result<(), Refusal> {
        let mut open = self.open.lock().await;
        let running = open.running.get(id).map(|running| running.handle.clone());
        let family = family(core, id, running.is_none()).await?;
        let parent = open.parent_of(core, id).await;
        if let Some(handle) = running {
            let stopped = match parent {
                Some(_) => handle.discard().await.map(Ok),
                None => handle.delete().await,
            };
            match stopped {
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
        if let Some((parent, job)) = parent {
            open.stopped_child(&parent, job).await;
        }
        // 重发的造会话不再交回删了的会话。
        open.created
            .retain(|(_, session)| session != id && !family.contains(session));
        let (root, account, at) = (core.root.clone(), core.admin.clone(), now());
        let index = Arc::clone(&core.index);
        let recall = Arc::clone(&core.memory.turns);
        let order: Vec<SessionId> = family.into_iter().rev().chain([id.clone()]).collect();
        let listing = Arc::clone(core);
        let moved = tokio::task::spawn_blocking(move || {
            order.into_iter().try_for_each(|session| {
                trash::discard(&root, &account, &session, at)
                    .map_err(|error| (session.clone(), error))?;
                // 挪走了才删那一行（施工 3-8 七补）：半路崩了，还在原处的照旧列得出来。
                forget(&index, &session);
                // 回合库里它的也拿掉（施工 R-2 上，`memory.md` 第一条第 7 款）：派生的，拿不掉只记一行，不挡删会话。
                if let Err(error) = recall.forget_session(&account, &session) {
                    tracing::warn!(target: "miyu::endpoint", session = session.as_str(), error = %error, "memory index not updated");
                }
                // 会话列表的推送（施工 9-5）：挪走一个报一个。
                listing.listing.removed(&listing, session);
                Ok(())
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

impl Open {
    /// 会话 `id` 是一个子会话、父会话还在的（施工 3-8 三补，施工 7-8 挪进表的锁里）：交回父会话和这个子代理的任务编号。父会话
    /// 照表的规矩找，没在跑的载入。不是子会话的、造它的命令编号读不出任务编号的、父会话已经不在（删了）或者载入不了的，没有：
    /// 删照常往下走。
    async fn parent_of(&mut self, core: &Arc<Core>, id: &SessionId) -> Option<(Handle, JobId)> {
        let dir = core.root.session_dir(&core.admin, id);
        let Ok(Ok(first)) = tokio::task::spawn_blocking(move || first_event(&dir)).await else {
            return None;
        };
        let (Body::SessionCreated(created), Some(cause)) = (first.body, first.cause) else {
            return None;
        };
        let parent = created.parent?;
        let job = job_in(&parent, &cause)?;
        let found = self.found(core, &parent, None, None).await.ok()?;
        Some((found.handle, job))
    }

    /// 父会话 `parent` 照人停掉它的子代理 `job` 记一条回报（`Handle::stopped_child`）：子会话已经停下了。它已经报过、被停过
    /// 的，父会话说已经结束了，不要紧：父会话早知道了。父会话停了的，从表里拿掉。
    async fn stopped_child(&mut self, parent: &Handle, job: JobId) {
        let job_text = job.to_string();
        match parent.stopped_child(job).await {
            Ok(Ok(())) => {
                tracing::debug!(target: "miyu::endpoint", session = parent.id().as_str(), job = job_text.as_str(), "stopped before deletion");
            }
            Ok(Err(_)) => {}
            Err(_) => {
                self.running.remove(parent.id());
            }
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
pub(super) fn descendants(
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

#[cfg(test)]
mod tests;

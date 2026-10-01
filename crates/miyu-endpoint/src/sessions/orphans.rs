//! 派到一半的空子会话（施工 7-8，`docs/blueprint/agents.md` 第一条第 7 条，`protocol.md`「会话表」第 8 条）：派子代理时
//! 子会话造好了，交代没送进去（调用交回派不了），或者父会话没来得及记下 `job.started` 就崩了，就留下一个父会话认不得的
//! 子会话。父会话载入时收掉：会话表里 `parent` 是它、它的日志里又没有这个子会话的 `job.started` 的，连同它们派的，停下、
//! 挪进回收处（照删会话，`delete.rs`，施工 3-8 三补）。
//!
//! 只在父会话的日志里有没派成的 `subagent` 调用（结果里没有 `job.started`，或者还没有结果）时才去认：认要把会话表里每个会话
//! 的第一条都读一遍，平常的载入不该为它慢下来。改名以前造的会话，日志里的调用叫 `agent`，一样认（施工 7-5 再补）。

use std::collections::BTreeSet;
use std::sync::Arc;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Effect};
use miyu_kernel::id::{AccountId, SessionId};
use miyu_store::log::{first_event, read_events};
use miyu_store::root::DataRoot;
use miyu_store::trash;
use miyu_tool::is_subagent;

use super::delete::descendants;
use super::{Open, now};
use crate::Core;
use crate::list::forget;

impl Open {
    /// 收掉会话 `parent` 派到一半的空子会话，表的锁在调的一方手里：在跑的停下（不问忙不忙，它们派的一起），目录挪进回收处，
    /// 最深的在前。认不出来、挪不走的记一行运行日志，不耽误载入。
    pub(super) async fn sweep_orphans(&mut self, core: &Core, parent: &SessionId) {
        let (root, account, id) = (core.root.clone(), core.admin.clone(), parent.clone());
        let orphans = match tokio::task::spawn_blocking(move || orphans(&root, &account, &id)).await
        {
            Ok(orphans) => orphans,
            Err(error) => {
                tracing::error!(target: "miyu::endpoint", error = %error, "orphan sweep panicked");
                return;
            }
        };
        if orphans.is_empty() {
            return;
        }
        for orphan in &orphans {
            if let Some(running) = self.running.remove(orphan)
                && running.handle.discard().await.is_err()
            {
                tracing::debug!(target: "miyu::endpoint", session = orphan.as_str(), "already stopped");
            }
        }
        self.created
            .retain(|(_, session)| !orphans.contains(session));
        let (root, account, at, parent) =
            (core.root.clone(), core.admin.clone(), now(), parent.clone());
        let index = Arc::clone(&core.index);
        let moved = tokio::task::spawn_blocking(move || {
            for orphan in orphans {
                match trash::discard(&root, &account, &orphan, at) {
                    Ok(()) => {
                        forget(&index, &orphan);
                        tracing::info!(target: "miyu::endpoint", session = orphan.as_str(), parent = parent.as_str(), "orphan subagent removed");
                    }
                    Err(error) => {
                        tracing::warn!(target: "miyu::endpoint", session = orphan.as_str(), error = %error, "orphan subagent not removed");
                    }
                }
            }
        })
        .await;
        if let Err(error) = moved {
            tracing::error!(target: "miyu::endpoint", error = %error, "orphan sweep panicked");
        }
    }
}

/// 在阻塞线程里认：`parent` 派到一半的空子会话，连同它们派的，最深的在前。父会话的日志读不出来、会话列不出来的，当没有。
fn orphans(root: &DataRoot, account: &AccountId, parent: &SessionId) -> Vec<SessionId> {
    let Ok(events) = read_events(&root.session_dir(account, parent)) else {
        return Vec::new();
    };
    let mut calls = BTreeSet::new();
    let mut started_calls = BTreeSet::new();
    let mut started = BTreeSet::new();
    for event in &events {
        match &event.body {
            Body::MessageAssistant(reply) => {
                calls.extend(reply.blocks.iter().filter_map(|block| match block {
                    Block::ToolCall(call) if is_subagent(&call.name) => Some(call.call_id),
                    _ => None,
                }))
            }
            Body::ToolResult(result) => {
                for effect in &result.effects {
                    if let Effect::JobStarted(job) = effect
                        && let Some(session) = &job.session
                    {
                        started_calls.insert(result.call_id);
                        started.insert(session.clone());
                    }
                }
            }
            _ => {}
        }
    }
    if calls.is_subset(&started_calls) {
        return Vec::new();
    }
    let Ok(ids) = root.sessions(account) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for id in ids.into_iter().filter(|id| !started.contains(id)) {
        let child_of_parent = first_event(&root.session_dir(account, &id)).is_ok_and(|event| {
            matches!(&event.body, Body::SessionCreated(created) if created.parent.as_ref() == Some(parent))
        });
        if child_of_parent {
            let below = descendants(root, account, &id, false).unwrap_or_default();
            found.extend(below.into_iter().rev());
            found.push(id);
        }
    }
    found
}

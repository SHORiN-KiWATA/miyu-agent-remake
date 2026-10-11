//! 找会话（`docs/designs/07-存储.md` 第七节「会话按需载入」）：在跑的直接交回，没在跑的从磁盘载入（施工 7-8 从
//! `sessions.rs` 挪来：删会话要拿着表的锁找父会话）。载入以后收掉派到一半的空子会话（`orphans.rs`）。

use std::sync::Arc;

use miyu_kernel::event::Body;
use miyu_kernel::id::{AccountId, SessionId};
use miyu_session::{Load, LoadError, load};
use miyu_store::log::{OpenError, read_events};

use super::{Found, Open, Running, environment, workspace};
use crate::Core;
use crate::list::{NO_CWD, cwd};
use crate::refusal::Refusal;
use crate::spawn;

impl Open {
    /// 找会话 `id`，表的锁在调的一方手里（[`super::Sessions::get`]）：在跑的直接交回；没在跑的从磁盘载入，载入以后收掉它
    /// 派到一半的空子会话（施工 7-8）。
    pub(super) async fn found(
        &mut self,
        core: &Arc<Core>,
        id: &SessionId,
    ) -> Result<Found, Refusal> {
        if let Some(running) = self.running.get(id) {
            return Ok(Found {
                handle: running.handle.clone(),
                cwd: running.workspace.clone(),
            });
        }
        // 照属主的家目录找（施工 O-4 上）：哪个账号的家目录下都没有的是没有这个会话。
        let Some(owner) = super::stored_owner(core, id).await else {
            return Err(Refusal::NOT_FOUND);
        };
        // 照日志里最后一次记下的工作目录，都没有才退回 `~`（施工 4-9 再补三上）；加进来的目录照最后一次记下的（施工 5-10
        // 上）。施工 9-7 上起换工作区的事件也算。
        let (last_cwd, dirs) = remembered(core, &owner, id).await;
        // 记下的是当时挑好的：太宽的是人选的、照用（施工 9-7 补：原来载入时再判一次，重启以后人选的 `~` 被换掉），落在
        // 数据根里的照旧退回；什么都没记下的照头自己带上的判。
        let workspace = match last_cwd {
            Some(cwd) => super::pick(core, &owner, &cwd, true).cwd,
            None => workspace(core, &owner, NO_CWD),
        };
        let loaded = load(Load {
            root: &core.root,
            personas: crate::personas::personas(core),
            resources: &core.resources,
            owner: owner.clone(),
            id: id.clone(),
            environment: environment(workspace.clone(), dirs.clone()),
            models: &*core.models,
            tools: &core.tools,
            home: core.home.as_deref(),
            sandbox: core.sandbox.helper(),
            sandbox_cache: core.sandbox_cache_of(&owner),
            sessions: Some(spawn::port(core)),
            jobs: &core.jobs,
            index: core.index_for(&owner),
            usage: core.usage_for(&owner),
            memory: core.memory_for(&owner),
            presets: Some(crate::presets::places(core)),
            owner_is_admin: owner == core.admin,
            configs: core.hub.configs(),
        })
        .await;
        let handle = match loaded {
            Ok(handle) => handle,
            Err(LoadError::Log(OpenError::Missing(_))) => return Err(Refusal::NOT_FOUND),
            Err(error) => {
                tracing::warn!(target: "miyu::endpoint", session = id.as_str(), error = %error, "load failed");
                return Err(Refusal::BROKEN);
            }
        };
        self.sweep_orphans(core, &owner, id).await;
        self.running.insert(
            id.clone(),
            Running {
                handle: handle.clone(),
                workspace: workspace.clone(),
                dirs,
            },
        );
        Ok(Found {
            handle,
            cwd: workspace,
        })
    }
}

/// 会话日志里最后一次记下的工作目录（施工 4-9 再补三上）：最后一条带 `cwd` 的 `turn.started`，没有就照
/// `session.created` 的；之前的日志没有这两格，是空的。列会话照同一个认法（[`cwd`]，施工 C-3）。加进来的目录照最后一条 `turn.started` 的，没有就是没有（施工
/// 5-10 上）。在阻塞线程里读。
async fn remembered(
    core: &Core,
    owner: &AccountId,
    id: &SessionId,
) -> (Option<String>, Vec<String>) {
    let dir = core.root.session_dir(owner, id);
    tokio::task::spawn_blocking(move || {
        let Ok(events) = read_events(&dir) else {
            return (None, Vec::new());
        };
        let cwd = events
            .iter()
            .rev()
            .find_map(|event| cwd(event).map(str::to_string));
        let dirs = events
            .iter()
            .rev()
            .find_map(|event| match &event.body {
                Body::TurnStarted(started) => Some(started.dirs.clone()),
                // 换工作区写了加进来的目录的（施工 9-7 上）；没写的照更早的。
                Body::WorkspaceChanged(changed) => changed.dirs.clone(),
                _ => None,
            })
            .unwrap_or_default();
        (cwd, dirs)
    })
    .await
    .unwrap_or_default()
}

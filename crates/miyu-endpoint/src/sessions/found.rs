//! 找会话（`docs/designs/07-存储.md` 第七节「会话按需载入」）：在跑的直接交回，没在跑的从磁盘载入（施工 7-8 从
//! `sessions.rs` 挪来：删会话要拿着表的锁找父会话）。载入以后在后台收掉派到一半的空子会话（`orphans.rs`）。整份日志只在
//! 载入时读一遍（施工 V-2 三补）：在哪干活交给载入顺手认，收空子会话不挡着载入。

use std::sync::Arc;

use miyu_kernel::id::SessionId;
use miyu_session::{Load, LoadError, Workplace, load_placed};
use miyu_store::log::OpenError;

use super::{Found, Open, Running, offset, workspace};
use crate::Core;
use crate::list::NO_CWD;
use crate::refusal::Refusal;
use crate::spawn;

impl Open {
    /// 找会话 `id`，表的锁在调的一方手里（[`super::Sessions::get`]）：在跑的直接交回；没在跑的从磁盘载入，载入以后在后台
    /// 收掉它派到一半的空子会话（施工 7-8，V-2 三补起不挡着载入）。
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
        // 上）。施工 9-7 上起换工作区的事件也算。记下的是当时挑好的：太宽的是人选的、照用（施工 9-7 补：原来载入时再判一次，
        // 重启以后人选的 `~` 被换掉），落在数据根里的照旧退回；什么都没记下的照头自己带上的判。施工 V-2 三补起载入读日志时
        // 顺手认，不另读一遍。
        let (picking, owner_of) = (Arc::clone(core), owner.clone());
        let pick = Box::new(move |last: Option<&str>| match last {
            Some(cwd) => super::pick(&picking, &owner_of, cwd, true).cwd,
            None => workspace(&picking, &owner_of, NO_CWD),
        });
        let loaded = load_placed(Load {
            root: &core.root,
            personas: crate::personas::personas(core),
            resources: &core.resources,
            owner: owner.clone(),
            id: id.clone(),
            place: Workplace::Remembered {
                offset: offset(),
                pick,
            },
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
        let (handle, placed) = match loaded {
            Ok(loaded) => loaded,
            Err(LoadError::Log(OpenError::Missing(_))) => return Err(Refusal::NOT_FOUND),
            Err(error) => {
                tracing::warn!(target: "miyu::endpoint", session = id.as_str(), error = %error, "load failed");
                return Err(Refusal::BROKEN);
            }
        };
        // 表里记的照载入挑定的：载入交回来，不拿着表的锁去问 actor（它头一批要做的事可能也要表的锁）。
        let workspace = placed.cwd;
        self.running.insert(
            id.clone(),
            Running {
                handle: handle.clone(),
                workspace: workspace.clone(),
                dirs: placed.dirs,
            },
        );
        super::orphans::sweep_later(core, owner, id.clone());
        Ok(Found {
            handle,
            cwd: workspace,
        })
    }
}

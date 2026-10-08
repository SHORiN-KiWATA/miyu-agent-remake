//! 换会话在哪个目录干活（施工 9-7 上，`docs/blueprint/protocol.md`「`session.set_workspace`」）：工作区是会话的属性，只有人
//! 明确换才变（网页里选、`/workspace`），一个头换了别的头跟着变（订阅着的收到 `session.workspace_changed`，会话列表那一项的
//! `cwd` 跟着变）。头每句话报的目录不再换它。
//!
//! 人明着换，写错了当场说，不悄悄退回：换不成真实位置、读不了的 `path_unreadable`，是文件的 `not_a_directory`，落在数据根里
//! 又不是账号自己的工作区的 `path_forbidden`。只有太宽的（系统的家目录、根目录、包含数据根的）照旧退回账号的工作区，回应写
//! 实际用的（2026-10-07 项目主人定：太宽照旧在换的时候判、回实际的）。加进来的目录照造会话的规矩查（`dir_too_wide`）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::SessionId;
use miyu_kernel::session::Command;

use crate::Core;
use crate::methods::command_to;
use crate::refusal::Refusal;
use crate::sessions::{check_dirs, workspace};
use crate::wire::Request;

/// `session.set_workspace` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetParams {
    /// 会话编号。
    session: String,
    /// 新的工作目录：绝对路径或者 `~` 开头的，人看到的那种写法；不写的照旧（只换加进来的目录）。
    #[serde(default)]
    cwd: Option<String>,
    /// 新的加进来的目录：写了的整份换掉，空的是去掉全部；不写的照旧。
    #[serde(default)]
    dirs: Option<Vec<String>>,
}

/// `session.set_workspace`：查过路径，交给会话换（和现在一样的什么都不记），会话表记着的跟着换。回应 `{"cwd", "dirs"}`：实际
/// 用的工作目录、现在加进来的目录。
pub(crate) async fn set(
    core: &Arc<Core>,
    request: &Request,
    params: SetParams,
) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    if params.cwd.is_none() && params.dirs.is_none() {
        return Err(Refusal::BAD_PARAMS);
    }
    let checked_cwd = params
        .cwd
        .as_deref()
        .map(|cwd| checked(core, cwd))
        .transpose()?;
    if let Some(dirs) = &params.dirs {
        check_dirs(core, dirs)?;
    }
    let found = core.sessions.get(core, &session).await?;
    // 没写工作目录的：照会话现在的。
    let cwd = checked_cwd.unwrap_or_else(|| found.cwd.clone());
    let command = Command::SetWorkspace {
        cwd: cwd.clone(),
        dirs: params.dirs.clone(),
    };
    command_to(core, request, &session, &found.handle, command).await?;
    let dirs = core
        .sessions
        .moved(&session, cwd.clone(), params.dirs)
        .await
        .unwrap_or_default();
    Ok(json!({"cwd": cwd, "dirs": dirs}))
}

/// 核心所在的机器上人的家目录，真实的位置；没有的、换不成的没有。
pub(crate) fn home(core: &Core) -> Option<PathBuf> {
    core.home
        .as_deref()
        .and_then(|home| std::fs::canonicalize(home).ok())
}

/// 查一个人明着要换去的工作目录，交回实际用的：见模块的说明。`/workspace` 也照它查（施工 9-7 下）。
pub(crate) fn checked(core: &Core, cwd: &str) -> Result<String, Refusal> {
    // `~` 本身总是太宽：读不出家目录也照造会话的退回，不当读不了。
    if cwd.trim() == "~" {
        return Ok(workspace(core, cwd));
    }
    let home = home(core);
    let real = miyu_fs::resolve(Path::new("/"), home.as_deref(), cwd)
        .map_err(|_| Refusal::PATH_UNREADABLE)?;
    let meta = std::fs::metadata(&real).map_err(|_| Refusal::PATH_UNREADABLE)?;
    if !meta.is_dir() {
        return Err(Refusal::NOT_A_DIRECTORY);
    }
    let data_root =
        std::fs::canonicalize(core.root.path()).unwrap_or_else(|_| core.root.path().to_path_buf());
    let own = core.root.workspace(&core.admin);
    let own = std::fs::canonicalize(&own).unwrap_or(own);
    if real.starts_with(&data_root) && !real.starts_with(&own) {
        return Err(Refusal::PATH_FORBIDDEN);
    }
    // 太宽的照旧退回账号的工作区；不太宽的照人写的原样。
    Ok(workspace(core, cwd))
}

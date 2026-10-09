//! 会话在哪干活（从 `sessions.rs` 挪出来，那边放不下了）：头报上来的工作目录太宽时退回属主的工作区，加进来的目录太宽的整条
//! 命令都不收（`11-权限与沙盒.md` 第四节，施工 4-3 下、5-10 上）。

use std::path::Path;

use miyu_kernel::id::AccountId;

use crate::Core;
use crate::refusal::Refusal;

/// 加进来的目录里有太宽的：整条命令都不收（施工 5-10 上）。
pub(crate) fn check_dirs(core: &Core, dirs: &[String]) -> Result<(), Refusal> {
    if dirs.iter().any(|dir| dir_too_wide(core, dir)) {
        Err(Refusal::DIR_TOO_WIDE)
    } else {
        Ok(())
    }
}

/// 加进来的一个目录太不太宽：和工作目录同一套（`~` 本身、系统的家目录、根目录、包含数据根的），另外落在数据根里的
/// 一律算太宽，账号的工作区也不例外：工作目录太宽时有地方可退，加进来的目录没有。换不成真实位置的照原样，边界表里
/// 那一片不算。
fn dir_too_wide(core: &Core, dir: &str) -> bool {
    if dir.trim() == "~" {
        return true;
    }
    let home = core
        .home
        .as_deref()
        .and_then(|home| std::fs::canonicalize(home).ok());
    let Ok(real) = miyu_fs::resolve(Path::new("/"), home.as_deref(), dir) else {
        return false;
    };
    let data_root =
        std::fs::canonicalize(core.root.path()).unwrap_or_else(|_| core.root.path().to_path_buf());
    miyu_fs::within(&real, &data_root)
        || miyu_fs::too_wide(&real, home.as_deref(), &data_root, &data_root)
}

/// 拿头报上来的 `cwd` 当工作区。太宽的（`~` 本身、系统的家目录、根目录，包含数据根或者落在数据根里），退回
/// 会话的属主 `owner` 的工作区 `home/<账号>/workspace/`（`11-权限与沙盒.md` 第四节，施工 4-3 下；施工 O-4 下起照属主，
/// 系统账号的场所会话退回它自己的）。换不成真实位置的照原样：说不清它宽不宽，用到时工具自己报错。
pub(crate) fn workspace(core: &Core, owner: &AccountId, cwd: &str) -> String {
    let own = core.root.workspace(owner);
    let fallback = || {
        // 建家目录时就建了；老的数据根里可能还没有，补上。建不了的照样退回：用到时工具自己报错。
        if let Err(error) = core.root.prepare_home(owner) {
            tracing::warn!(target: "miyu::endpoint", kind = ?error.kind(), "workspace not prepared");
        }
        own.to_string_lossy().into_owned()
    };
    if cwd.trim() == "~" {
        return fallback();
    }
    let home = core
        .home
        .as_deref()
        .and_then(|home| std::fs::canonicalize(home).ok());
    let Ok(real) = miyu_fs::resolve(Path::new("/"), home.as_deref(), cwd) else {
        return cwd.to_string();
    };
    let data_root =
        std::fs::canonicalize(core.root.path()).unwrap_or_else(|_| core.root.path().to_path_buf());
    let own_real = std::fs::canonicalize(&own).unwrap_or_else(|_| own.clone());
    if miyu_fs::too_wide(&real, home.as_deref(), &data_root, &own_real) {
        fallback()
    } else {
        cwd.to_string()
    }
}

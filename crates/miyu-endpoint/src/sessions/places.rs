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

/// 挑好的工作区：实际用的目录，和它是不是太宽（施工 9-7 补）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Picked {
    /// 实际用的目录：照原样的写法，退回的是属主的工作区。
    pub(crate) cwd: String,
    /// 太宽（`~` 本身、系统的家目录、根目录、包含数据根的），照人选的用着。
    pub(crate) wide: bool,
}

/// 拿头报上来的 `cwd` 当工作区，同 [`pick`]、不是人明着选的：太宽的退回属主的工作区。
pub(crate) fn workspace(core: &Core, owner: &AccountId, cwd: &str) -> String {
    pick(core, owner, cwd, false).cwd
}

/// 照 `cwd` 挑工作区（`11-权限与沙盒.md` 第四节，施工 4-3 下；施工 O-4 下起照属主 `owner`，系统账号的场所会话退回它自己的）。
/// 落在数据根里、又不在属主的工作区 `home/<账号>/workspace/` 里的，退回属主的工作区。太宽的：人明着选的（`chosen`：
/// `session.create` 带 `chosen`、`session.set_workspace`、`/workspace`）照用、标上 `wide`（施工 9-7 补，2026-10-09 项目主人：
/// 「当然是我手动选择的，或者手动运行命令切换的，就别自动切工作区了」）；头自己带上的（终端启动时的当前目录）照旧退回。
/// 换不成真实位置的照原样：说不清它宽不宽，用到时工具自己报错；`~` 本身读不出家目录的照旧退回。
pub(crate) fn pick(core: &Core, owner: &AccountId, cwd: &str, chosen: bool) -> Picked {
    let own = core.root.workspace(owner);
    let fallback = || {
        // 建家目录时就建了；老的数据根里可能还没有，补上。建不了的照样退回：用到时工具自己报错。
        if let Err(error) = core.root.prepare_home(owner) {
            tracing::warn!(target: "miyu::endpoint", kind = ?error.kind(), "workspace not prepared");
        }
        Picked {
            cwd: own.to_string_lossy().into_owned(),
            wide: false,
        }
    };
    let home = core
        .home
        .as_deref()
        .and_then(|home| std::fs::canonicalize(home).ok());
    let Ok(real) = miyu_fs::resolve(Path::new("/"), home.as_deref(), cwd) else {
        // `~` 本身读不出家目录的：说不清在哪，照旧退回。
        return match cwd.trim() == "~" {
            true => fallback(),
            false => Picked {
                cwd: cwd.to_string(),
                wide: false,
            },
        };
    };
    let data_root =
        std::fs::canonicalize(core.root.path()).unwrap_or_else(|_| core.root.path().to_path_buf());
    let own_real = std::fs::canonicalize(&own).unwrap_or_else(|_| own.clone());
    if miyu_fs::within(&real, &data_root) && !miyu_fs::within(&real, &own_real) {
        return fallback();
    }
    let wide = miyu_fs::too_wide(&real, home.as_deref(), &data_root, &own_real);
    match (wide, chosen) {
        (true, false) => fallback(),
        (wide, _) => Picked {
            cwd: cwd.to_string(),
            wide,
        },
    }
}

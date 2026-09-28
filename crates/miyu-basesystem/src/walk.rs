//! 找文件、搜内容往下走目录（`10-自带软件.md` 第三节「`glob`、`grep` 输出的写法」，施工 4-4 下）。
//!
//! - 遵守 `.gitignore`、`.ignore`。在 git 仓库里时，仓库根到搜的那个目录之间的也算；不在仓库里时，只认搜的那个
//!   目录往下的：家目录里要是有一份写着 `*` 的 `.gitignore`（拿 git 管 dotfiles 的常这么配），往上找就什么都搜
//!   不到了。pi 也是这样分的。
//! - 隐藏文件照找；版本库自己的目录（[`VCS`]）不进；不跟着链接走。
//! - Miyu 的数据根不进（[`Fence`]）：权限策略只核对搜的那个目录，从上面搜下来会走进去（`11-权限与沙盒.md` A9）。
//!   这一轮的工作目录在数据根里面的（账号自己的工作区就在数据根里），照样进，和权限策略的先后一样。
//! - 按修改时间排，新的在前，时间一样的照路径排。

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ignore::WalkBuilder;

use miyu_fs::{resolve, within};
use miyu_tool::{Call, Stop};

/// 版本库自己的目录，不进（Claude Code 的名单）。
const VCS: [&str; 6] = [".git", ".svn", ".hg", ".bzr", ".jj", ".sl"];

/// 走到的一个文件。
#[derive(Debug)]
pub(crate) struct Found {
    /// 真实的位置。
    pub(crate) path: PathBuf,
    /// 相对搜的那个目录的路径，分隔符一律写 `/`：照它比模式。
    pub(crate) relative: String,
    /// 修改时间：读不出来的当最早。
    modified: SystemTime,
}

/// 走目录时不进的地方：Miyu 的数据根，除了这一轮的工作目录那一片。
pub(crate) struct Fence {
    data_root: Option<PathBuf>,
    workspace: Option<PathBuf>,
}

impl Fence {
    /// 照调用 `call`：数据根和工作目录都换成真实的位置，换不成的不算。
    pub(crate) fn of(call: &Call) -> Fence {
        Fence {
            data_root: call
                .data_root
                .as_deref()
                .and_then(|dir| std::fs::canonicalize(dir).ok()),
            workspace: resolve(Path::new(&call.cwd), call.home.as_deref(), &call.cwd).ok(),
        }
    }

    /// 目录 `dir` 不进：在数据根里，又不在工作目录那一片里。
    fn keeps_out(&self, dir: &Path) -> bool {
        self.data_root
            .as_deref()
            .is_some_and(|data_root| within(dir, data_root))
            && !self
                .workspace
                .as_deref()
                .is_some_and(|workspace| within(dir, workspace))
    }
}

/// 在目录 `root`（已经换成真实的位置）下面走一遍，交回 `keep` 留下的每个普通文件，按修改时间新的在前。
/// `keep` 拿到的是相对 `root`、分隔符写 `/` 的路径。`fence` 挡着的目录不进；叫停了（`stop`）就不往下走了。
pub(crate) fn files(
    root: &Path,
    fence: Fence,
    stop: &Stop,
    keep: impl Fn(&str) -> bool,
) -> Vec<Found> {
    let in_repo = root.ancestors().any(|dir| dir.join(".git").exists());
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .follow_links(false)
        .parents(in_repo)
        .require_git(in_repo)
        .filter_entry(move |entry| {
            let dir = entry.file_type().is_some_and(|kind| kind.is_dir());
            if !dir || entry.depth() == 0 {
                return true;
            }
            let vcs = VCS.contains(&entry.file_name().to_string_lossy().as_ref());
            !vcs && !fence.keeps_out(entry.path())
        });
    let mut found: Vec<Found> = builder
        .build()
        .take_while(|_| !stop.stopped())
        // 读不了的目录（没有权限这类）跳过，别的照走：ripgrep 也是这样。
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let relative = relative(root, entry.path())?;
            if !keep(&relative) {
                return None;
            }
            let modified = entry
                .metadata()
                .and_then(|meta| meta.modified().map_err(ignore::Error::from))
                .unwrap_or(SystemTime::UNIX_EPOCH);
            Some(Found {
                path: entry.into_path(),
                relative,
                modified,
            })
        })
        .collect();
    found.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| a.relative.cmp(&b.relative))
    });
    found
}

/// `path` 相对 `root` 的路径，一段一段用 `/` 连起来。
fn relative(root: &Path, path: &Path) -> Option<String> {
    let rest = path.strip_prefix(root).ok()?;
    let parts: Vec<String> = rest
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests;

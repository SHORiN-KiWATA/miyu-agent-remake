//! 核心报的文件在哪（`config.md` 第十条）：数据根里的写成相对数据根的，项目配置写成 `~/…` 或者绝对路径；命令行照
//! 数据根、家目录换成真的位置，印的时候家目录下的写成 `~/…`。

use std::path::{Path, PathBuf};

use super::ConfigPlan;
use crate::shown::tilde;

/// 换路径要的两样。
pub(super) struct Places {
    root: PathBuf,
    home: Option<PathBuf>,
}

impl Places {
    /// 照这一次的数据根、家目录。
    pub(super) fn of(plan: &ConfigPlan) -> Places {
        Places {
            root: plan.root.clone(),
            home: plan.home.clone(),
        }
    }

    /// 核心报的 `file` 的真的位置：`~`、`~/` 开头的照家目录换开，绝对路径照原样，别的接在数据根后面。
    pub(super) fn absolute(&self, file: &str) -> PathBuf {
        if let (Some(rest), Some(home)) = (home_part(file), &self.home) {
            return rest
                .split('/')
                .filter(|part| !part.is_empty())
                .fold(home.clone(), |path, part| path.join(part));
        }
        let path = Path::new(file);
        match path.is_absolute() {
            true => path.to_path_buf(),
            false => file
                .split('/')
                .fold(self.root.clone(), |path, part| path.join(part)),
        }
    }

    /// 印的时候的写法：真的位置，家目录下的写成 `~/…`（报错一行的开头、`explain` 的每一层）。
    pub(super) fn shown(&self, file: &str) -> String {
        tilde(&self.absolute(file).to_string_lossy(), self.home.as_deref())
    }
}

/// `~` 开头的：`~` 后面那一截。
fn home_part(file: &str) -> Option<&str> {
    match file {
        "~" => Some(""),
        _ => file.strip_prefix("~/"),
    }
}

/// 还没有项目配置时它该在哪（`path --project`）：从 `cwd` 往上，有 `.git` 的那一层是仓库的根，没有的就是 `cwd`，
/// 再接 `.miyu/config.toml`。
pub(super) fn planned(cwd: &Path) -> PathBuf {
    let root = cwd
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .unwrap_or(cwd);
    root.join(".miyu").join("config.toml")
}

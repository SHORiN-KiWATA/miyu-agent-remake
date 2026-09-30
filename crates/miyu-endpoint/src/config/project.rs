//! 找项目配置（`docs/blueprint/config.md`「怎么走」第三条第 1 条，G3）：从目录起往上一层层找 `.miyu/config.toml`，
//! 最近的那一份就是，只认一份，不叠。
//!
//! - 找到有 `.git`（目录、文件都算）的那一层就停：那是仓库的根。
//! - 到了系统的家目录就停，家目录本身不看；到了根目录也停，根目录本身也不看。
//! - 落在数据根里的目录不找：`~/.miyu` 不会被当成一个仓库的 `.miyu`。
//! - 每一层只看在不在，读的时候照配置文件的规矩读。

use std::path::{Path, PathBuf};

/// 仓库里项目配置的位置：`.miyu/config.toml`。
pub(crate) const RELATIVE: [&str; 2] = [".miyu", "config.toml"];

/// 从真实的目录 `start` 往上找项目配置，交回 `.miyu` 所在的那一层。`home` 是系统的家目录，`data_root` 是数据根，
/// 都是真实的位置。
pub(crate) fn find(start: &Path, home: Option<&Path>, data_root: &Path) -> Option<PathBuf> {
    if miyu_fs::within(start, data_root) {
        return None;
    }
    let mut dir = start;
    loop {
        if home == Some(dir) {
            return None;
        }
        let parent = dir.parent()?;
        if config_in(dir).exists() {
            return Some(dir.to_path_buf());
        }
        if dir.join(".git").exists() {
            return None;
        }
        dir = parent;
    }
}

/// 仓库 `repo` 里项目配置的路径。
pub(crate) fn config_in(repo: &Path) -> PathBuf {
    RELATIVE
        .iter()
        .fold(repo.to_path_buf(), |path, part| path.join(part))
}

/// 给人看的路径：家目录下的写成 `~/…`（`/` 分段），别的照原样；Windows 上去掉真实位置开头的 `\\?\`。
pub(crate) fn shown(path: &Path, home: Option<&Path>) -> String {
    if let Some(rest) = home.and_then(|home| path.strip_prefix(home).ok()) {
        let parts: Vec<String> = rest
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        return match parts.is_empty() {
            true => "~".to_string(),
            false => format!("~/{}", parts.join("/")),
        };
    }
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

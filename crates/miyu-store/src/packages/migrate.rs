//! 家目录里以前的写法挪成新的（施工 F-8 上，设计 `31-软件包.md` 第七节第 2 条）：以前一个包是 `<编号>.toml` 加同名目录，
//! 现在一个文件夹就是一个包，清单是文件夹里的 `package.toml`。核心起来、重读清单时挪一次；挪过的不再有 `<编号>.toml`，再来
//! 一次什么都不做。出厂那一层在仓库里挪好，不归这里管。

use std::fs;
use std::io;
use std::path::Path;

use super::MANIFEST;

/// 挪了的一个包。
#[derive(Debug)]
pub struct Moved {
    /// 编号。
    pub id: String,
    /// 挪成了，或者为什么没挪。
    pub result: Result<(), Unmoved>,
}

/// 没挪的原因。
#[derive(Debug)]
pub enum Unmoved {
    /// 文件夹里已经有一份 `package.toml`：两份都留着，不覆盖，交给人看。
    Both,
    /// 挪不动。
    Io(io::Error),
}

/// 把家目录这一层 `home`（`home/<管理员>/packages/`）里以前的写法挪成新的：`<编号>.toml` 挪进同名文件夹、改名
/// `package.toml`，文件夹没有的建一个。编号不合写法的、点开头的不动。目录读不了的当没有，交回空的。
pub fn old_layout(home: &Path) -> Vec<Moved> {
    let Ok(entries) = fs::read_dir(home) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let id = name.strip_suffix(".toml")?;
            crate::personas::valid(id).then(|| id.to_string())
        })
        .collect();
    ids.sort();
    ids.into_iter()
        .map(|id| {
            let result = move_one(home, &id);
            Moved { id, result }
        })
        .collect()
}

/// 挪一个：文件夹里已经有清单的不动。
fn move_one(home: &Path, id: &str) -> Result<(), Unmoved> {
    let folder = home.join(id);
    let target = folder.join(MANIFEST);
    if fs::symlink_metadata(&target).is_ok() {
        return Err(Unmoved::Both);
    }
    fs::create_dir_all(&folder).map_err(Unmoved::Io)?;
    fs::rename(home.join(format!("{id}.toml")), &target).map_err(Unmoved::Io)
}

#[cfg(test)]
mod tests;

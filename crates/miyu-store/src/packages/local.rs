//! 本地库（施工 F-8 中上，设计 `31-软件包.md` 第四节，照 pacman 的 `local/`）：家目录里装的每个包记一份，放在
//! `<数据根>/state/packages/.local/<编号>/`（点开头：和包自己放状态的 `state/packages/<编号>/` 在同一层，编号不会撞上）。
//!
//! - `desc`：一个 JSON 对象，编号、版本（清单写了的）、装的时刻、从哪装的（核心起来时补的是空的）、总字节数。
//! - `files`：一个文件一行 `<SHA-256> <字节数> <相对包目录的路径>`，路径用 `/` 分开，照路径排；路径放最后，带空格也认得。
//!
//! 出厂的包不进本地库：资源目录归系统的包管理。写的时候整份先写进点开头的暂存处再换上。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 本地库在状态目录 `state/packages/` 下的名字。
pub const LOCAL: &str = ".local";

/// 一个包的信息（`desc`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Desc {
    /// 编号。
    pub id: String,
    /// 版本，清单写了的才有；只给人看，不比大小。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// 装的时刻（内核时刻的写法）。
    pub installed: String,
    /// 从哪个路径装的；核心起来时给以前装的补的是空的。
    #[serde(default)]
    pub source: String,
    /// 包目录里所有文件加起来多少字节。
    pub size: u64,
}

/// 包目录里的一个文件（`files` 的一行）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 相对包目录的路径，用 `/` 分开。
    pub path: String,
    /// 内容的 SHA-256，64 位小写十六进制。
    pub sha256: String,
    /// 字节数。
    pub size: u64,
}

/// 扫一遍包目录 `folder`：每个文件的路径、哈希、大小，照路径排。链接照它指的读（装的时候已经照内容拷过来了）。
///
/// # Errors
///
/// 目录、文件读不了。
pub fn scan(folder: &Path) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    walk(folder, "", &mut entries)?;
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(entries)
}

fn walk(dir: &Path, prefix: &str, out: &mut Vec<Entry>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = match prefix {
            "" => name,
            _ => format!("{prefix}/{name}"),
        };
        if fs::metadata(entry.path())?.is_dir() {
            walk(&entry.path(), &path, out)?;
        } else {
            let bytes = fs::read(entry.path())?;
            out.push(Entry {
                path,
                sha256: hex(&Sha256::digest(&bytes)),
                size: bytes.len() as u64,
            });
        }
    }
    Ok(())
}

/// 小写十六进制。
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 本地库 `root`（`state/packages/.local`）里记下包 `desc.id`：整份先写进 `.<编号>.new` 再换上。
///
/// # Errors
///
/// 建不了目录、写不进、换不上。
pub fn write(root: &Path, desc: &Desc, files: &[Entry]) -> io::Result<()> {
    fs::create_dir_all(root)?;
    let staged = root.join(format!(".{}.new", desc.id));
    if staged.exists() {
        fs::remove_dir_all(&staged)?;
    }
    fs::create_dir_all(&staged)?;
    let json = serde_json::to_string(desc).map_err(io::Error::other)?;
    fs::write(staged.join("desc"), json + "\n")?;
    let lines: String = files
        .iter()
        .map(|file| format!("{} {} {}\n", file.sha256, file.size, file.path))
        .collect();
    fs::write(staged.join("files"), lines)?;
    let target = root.join(&desc.id);
    if target.exists() {
        fs::remove_dir_all(&target)?;
    }
    fs::rename(&staged, &target)
}

/// 读本地库 `root` 里的包 `id`；没记的、读不懂的没有。
pub fn read(root: &Path, id: &str) -> Option<(Desc, Vec<Entry>)> {
    let dir = root.join(id);
    let desc: Desc = serde_json::from_str(&fs::read_to_string(dir.join("desc")).ok()?).ok()?;
    let files = fs::read_to_string(dir.join("files"))
        .ok()?
        .lines()
        .map(line)
        .collect::<Option<Vec<Entry>>>()?;
    Some((desc, files))
}

/// `files` 的一行。
fn line(text: &str) -> Option<Entry> {
    let mut parts = text.splitn(3, ' ');
    let sha256 = parts.next()?.to_string();
    let size = parts.next()?.parse().ok()?;
    let path = parts.next()?.to_string();
    (sha256.len() == 64 && !path.is_empty()).then_some(Entry { path, sha256, size })
}

/// 删掉本地库 `root` 里的包 `id`；没记的不算错。
///
/// # Errors
///
/// 删不掉。
pub fn remove(root: &Path, id: &str) -> io::Result<()> {
    match fs::remove_dir_all(root.join(id)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// 本地库 `root` 里记了哪几个包，照编号排。目录读不了的当没有。
pub fn ids(root: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|id| crate::personas::valid(id))
        .collect();
    ids.sort();
    ids
}

/// 一个包的总字节数。
pub fn size(files: &[Entry]) -> u64 {
    files.iter().map(|file| file.size).sum()
}

/// 状态目录 `state` 下本地库在哪。
pub fn root(state: &Path) -> PathBuf {
    state.join(LOCAL)
}

#[cfg(test)]
mod tests;

//! Linux 的回收站（freedesktop.org 回收站规范 1.0，施工 4-6 下）：只碰文件，不依赖桌面。
//!
//! - 和家目录的回收站（`$XDG_DATA_HOME/Trash`，没设的是 `~/.local/share/Trash`）在同一块盘上的，放那里；
//! - 不在的，放那块盘最上面那一层目录下的 `.Trash/<uid>`（`.Trash` 在、是目录、带粘滞位的时候）或者 `.Trash-<uid>`；
//! - 先用只许新建的方式写好 `info/<名字>.trashinfo`（原来的位置、删的时间），再把东西改名挪进 `files/<名字>`，重名的接
//!   `.2`、`.3`；挪不进去的，删掉那份 `.trashinfo`。
//!
//! 放不进去的（那块盘上建不了回收站）不删：不像 `trash` 这个 crate 那样拷一份再删原来的，拷过去的撤销时移不回来。

use std::ffi::OsStr;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use super::Refused;

/// 重名的最多接到几。
const TRIES: u32 = 1000;

/// 把 `real` 放进回收站，交回它在回收站里的位置：`files/<名字>` 的绝对路径。`home` 是交给工具的家目录。
pub(super) fn put(real: &Path, home: Option<&Path>) -> Result<String, Refused> {
    let meta = fs::symlink_metadata(real).map_err(Refused::Failed)?;
    let device = meta.dev();
    let (trash, top) = match home_trash(home) {
        Some(dir) if device_of(&dir) == Some(device) => (dir, None),
        _ => {
            let top = top_of(real, device);
            (top_trash(&top)?, Some(top))
        }
    };
    let (files, info) = (trash.join("files"), trash.join("info"));
    for dir in [&files, &info] {
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(|_| Refused::Unavailable)?;
    }
    let name = real
        .file_name()
        .ok_or_else(|| Refused::Failed(io::Error::from(io::ErrorKind::InvalidInput)))?;
    let original = match &top {
        Some(top) => real.strip_prefix(top).unwrap_or(real),
        None => real,
    };
    let record = trash_info(original);
    for n in 1..=TRIES {
        let chosen = match n {
            1 => name.to_os_string(),
            _ => {
                let mut chosen = name.to_os_string();
                chosen.push(format!(".{n}"));
                chosen
            }
        };
        let mut record_name = chosen.clone();
        record_name.push(".trashinfo");
        let record_path = info.join(&record_name);
        let mut file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&record_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(Refused::Failed(error)),
        };
        let target = files.join(&chosen);
        let written = file
            .write_all(record.as_bytes())
            .and_then(|()| file.sync_all());
        drop(file);
        if fs::symlink_metadata(&target).is_ok() {
            remove(&record_path);
            continue;
        }
        if let Err(error) = written.and_then(|()| fs::rename(real, &target)) {
            remove(&record_path);
            return Err(match error.kind() {
                // 跨了盘，挪不过去：当收不了。
                io::ErrorKind::CrossesDevices => Refused::Unavailable,
                _ => Refused::Failed(error),
            });
        }
        return Ok(target.to_string_lossy().into_owned());
    }
    Err(Refused::Failed(io::Error::from(
        io::ErrorKind::AlreadyExists,
    )))
}

/// 家目录的回收站：交给工具的家目录就是这个进程的家目录时，照 `XDG_DATA_HOME`；不然照那个家目录下的
/// `.local/share`。`XDG_DATA_HOME` 说的是这个进程的家：测试里的假家目录不会碰到真的回收站。
fn home_trash(home: Option<&Path>) -> Option<PathBuf> {
    let home = home?;
    let own = std::env::var_os("HOME").is_some_and(|own| Path::new(&own) == home);
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|data| own && data.is_absolute())
        .unwrap_or_else(|| home.join(".local").join("share"));
    Some(data.join("Trash"))
}

/// 一个位置在哪块盘上：还不存在的，照它最近的一层已经在的上级算。
fn device_of(path: &Path) -> Option<u64> {
    path.ancestors()
        .find_map(|dir| fs::metadata(dir).ok())
        .map(|meta| meta.dev())
}

/// 这块盘最上面那一层目录：从 `real` 的上级往上走，走到上一层换了盘为止。
fn top_of(real: &Path, device: u64) -> PathBuf {
    let mut top = real.parent().unwrap_or(real).to_path_buf();
    while let Some(up) = top.parent() {
        match fs::metadata(up) {
            Ok(meta) if meta.dev() == device => top = up.to_path_buf(),
            _ => break,
        }
    }
    top
}

/// 那块盘上的回收站：`.Trash/<uid>`（`.Trash` 在、是目录、不是链接、带粘滞位），不然 `.Trash-<uid>`。建不了、
/// 不是自己的、是链接的，当收不了。
fn top_trash(top: &Path) -> Result<PathBuf, Refused> {
    let uid = fs::metadata("/proc/self").map_err(Refused::Failed)?.uid();
    let shared = top.join(".Trash");
    if let Ok(meta) = fs::symlink_metadata(&shared)
        && meta.is_dir()
        && meta.permissions().mode() & 0o1000 != 0
    {
        let mine = shared.join(uid.to_string());
        if usable(&mine, uid) {
            return Ok(mine);
        }
    }
    let mine = top.join(format!(".Trash-{uid}"));
    if usable(&mine, uid) {
        Ok(mine)
    } else {
        Err(Refused::Unavailable)
    }
}

/// `dir` 能不能当自己的回收站用：没有的建上（只有自己能进），有的要是目录、不是链接、是自己的。
fn usable(dir: &Path, uid: u32) -> bool {
    if fs::symlink_metadata(dir).is_err() && DirBuilder::new().mode(0o700).create(dir).is_err() {
        return false;
    }
    fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir() && meta.uid() == uid)
}

/// `.trashinfo` 的内容：原来的位置照规范转义，删的时间是本地时间，精确到秒。
fn trash_info(original: &Path) -> String {
    let when = jiff::Zoned::now().strftime("%Y-%m-%dT%H:%M:%S");
    format!(
        "[Trash Info]\nPath={}\nDeletionDate={when}\n",
        escaped(original.as_os_str())
    )
}

/// 规范要求的转义：字母、数字、`-._~/` 照写，别的每个字节写成 `%XX`。
fn escaped(path: &OsStr) -> String {
    let mut out = String::new();
    for &byte in path.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(char::from(byte));
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// 移回来了（施工 4-7 上）：删掉它的 `.trashinfo`。`kept` 是 `files/<名字>`，记录是同一个回收站里的
/// `info/<名字>.trashinfo`；不在 `files/` 下的，没有要删的。
pub(super) fn forget(kept: &Path) {
    let (Some(files), Some(name)) = (kept.parent(), kept.file_name()) else {
        return;
    };
    if files.file_name() != Some(OsStr::new("files")) {
        return;
    }
    let Some(trash) = files.parent() else {
        return;
    };
    let mut record = name.to_os_string();
    record.push(".trashinfo");
    remove(&trash.join("info").join(record));
}

/// 删掉用不着的 `.trashinfo`。删不掉的记一条运行日志：东西没动，回收站里多了一份空的记录。
fn remove(record: &Path) {
    if let Err(error) = fs::remove_file(record)
        && error.kind() != io::ErrorKind::NotFound
    {
        tracing::warn!(target: "miyu::fs", error = %error, "trash record left behind");
    }
}

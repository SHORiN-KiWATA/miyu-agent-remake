//! 把一份文件整体换成新的内容（`10-自带软件.md` 第三节「`write` 的细则」，施工 4-6 上）：先写进同一个目录里的
//! 临时文件、同步，再改名盖上去。中途崩了，原来的文件还在，不会留下写了一半的；原来的文件权限照留。`write`、
//! `edit`（4-6 中）和撤销时写回改前的内容（4-7 上）都用；施工 4-7 上从基础系统挪到这里。
//!
//! 原来是只读的不写：她碰到的是一个明摆着不让改的文件。Windows 上改名也盖不过只读的文件，三个平台照这一条一样。

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// 临时文件的名字撞上了，最多换几次。
const TRIES: u32 = 16;

/// 把 `real` 换成 `bytes`：原来有的照它的权限，没有的照系统默认的建。上级目录要已经在。
///
/// # Errors
///
/// 原来的是只读的（权限不够）；临时文件建不了、写不进、同步不了；改名盖不上去。没盖上去的，临时文件删掉。
pub fn replace(real: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = real
        .parent()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let permissions = match fs::metadata(real) {
        Ok(meta) if meta.permissions().readonly() => {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        Ok(meta) => Some(meta.permissions()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let (temp, mut file) = temp_in(dir, real)?;
    let written = file
        .write_all(bytes)
        .and_then(|()| match &permissions {
            Some(permissions) => file.set_permissions(permissions.clone()),
            None => Ok(()),
        })
        .and_then(|()| file.sync_all());
    drop(file);
    if let Err(error) = written.and_then(|()| fs::rename(&temp, real)) {
        remove(&temp);
        return Err(error);
    }
    Ok(())
}

/// 在 `dir` 里建一个新的临时文件，名字照 `real` 起：以点开头，带进程号和序号，撞上了就换。
fn temp_in(dir: &Path, real: &Path) -> io::Result<(PathBuf, File)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let name = real
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let mut last = io::Error::from(io::ErrorKind::AlreadyExists);
    for _ in 0..TRIES {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let temp = dir.join(format!(".{name}.{}-{n}.miyu-tmp", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => return Ok((temp, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => last = error,
            Err(error) => return Err(error),
        }
    }
    Err(last)
}

/// 删掉没用上的临时文件。删不掉的只记一条运行日志：原来的文件没动，只是目录里多了一个以点开头的临时文件。
fn remove(temp: &Path) {
    if let Err(error) = fs::remove_file(temp) {
        tracing::warn!(target: "miyu::fs", error = %error, "temporary file left behind");
    }
}

#[cfg(test)]
mod tests;

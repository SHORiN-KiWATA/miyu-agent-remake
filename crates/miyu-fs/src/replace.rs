//! 把一份文件整体换成新的内容（`10-自带软件.md` 第三节「`write` 的细则」，施工 4-6 上）：先写进同一个目录里的
//! 临时文件、同步，再改名盖上去。中途崩了，原来的文件还在，不会留下写了一半的；原来的文件权限照留。`write`、
//! `edit`（4-6 中）和撤销时写回改前的内容（4-7 上）都用；施工 4-7 上从基础系统挪到这里。
//!
//! 原来是只读的不写：她碰到的是一个明摆着不让改的文件。Windows 上改名也盖不过只读的文件，三个平台照这一条一样。
//!
//! Unix 上先把上级目录路上一层链接都不跟地打开，看原来的文件、建临时文件、改名、删临时文件都相对它做：检查完以后
//! 上级目录被换成了链接，写不到别处去（施工 5-10 下）。Windows 上照路径做。

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(unix))]
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
};

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
    #[cfg(unix)]
    {
        let name = real
            .file_name()
            .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
        at::replace(dir, name, bytes)
    }
    #[cfg(not(unix))]
    by_path(dir, real, bytes)
}

/// 照路径做：Windows 上。
#[cfg(not(unix))]
fn by_path(dir: &Path, real: &Path, bytes: &[u8]) -> io::Result<()> {
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

/// 临时文件的名字：以点开头，照原来的名字 `name` 起，带进程号和这个进程里的序号。
fn temp_name(name: &std::ffi::OsStr) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    format!(
        ".{}.{}-{n}.miyu-tmp",
        name.to_string_lossy(),
        std::process::id()
    )
}

/// 在 `dir` 里建一个新的临时文件，名字照 `real` 起，撞上了就换。
#[cfg(not(unix))]
fn temp_in(dir: &Path, real: &Path) -> io::Result<(PathBuf, File)> {
    let name = real.file_name().unwrap_or_default();
    let mut last = io::Error::from(io::ErrorKind::AlreadyExists);
    for _ in 0..TRIES {
        let temp = dir.join(temp_name(name));
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => return Ok((temp, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => last = error,
            Err(error) => return Err(error),
        }
    }
    Err(last)
}

/// 删掉没用上的临时文件。删不掉的只记一条运行日志：原来的文件没动，只是目录里多了一个以点开头的临时文件。
#[cfg(not(unix))]
fn remove(temp: &Path) {
    if let Err(error) = fs::remove_file(temp) {
        tracing::warn!(target: "miyu::fs", error = %error, "temporary file left behind");
    }
}

/// Unix 上相对打开了的上级目录做（施工 5-10 下）。
#[cfg(unix)]
mod at {
    use std::ffi::OsStr;
    use std::fs::{File, Permissions};
    use std::io::{self, Write};
    use std::os::fd::OwnedFd;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    use rustix::fs::{AtFlags, Mode, OFlags, openat, renameat, statat, unlinkat};
    use rustix::io::Errno;

    use super::{TRIES, temp_name};

    /// 把上级目录 `dir` 里的 `name` 换成 `bytes`。
    pub(super) fn replace(dir: &Path, name: &OsStr, bytes: &[u8]) -> io::Result<()> {
        let dir = crate::nofollow::open_dir(dir)?;
        // 看原来的文件，跟着链接：照它的权限写，一个写位都没有的不写。
        let mode = match statat(&dir, name, AtFlags::empty()) {
            Ok(stat) => {
                #[allow(
                    clippy::useless_conversion,
                    reason = "st_mode 在 Linux 上是 u32，在 macOS 上是 u16"
                )]
                let mode = u32::from(stat.st_mode) & 0o7777;
                if mode & 0o222 == 0 {
                    return Err(io::Error::from(io::ErrorKind::PermissionDenied));
                }
                Some(mode)
            }
            Err(Errno::NOENT) => None,
            Err(error) => return Err(error.into()),
        };
        let (temp, fd) = temp_in(&dir, name)?;
        let mut file = File::from(fd);
        let written = file
            .write_all(bytes)
            .and_then(|()| match mode {
                Some(mode) => file.set_permissions(Permissions::from_mode(mode)),
                None => Ok(()),
            })
            .and_then(|()| file.sync_all());
        drop(file);
        let placed = written.and_then(|()| renameat(&dir, &temp, &dir, name).map_err(Into::into));
        if let Err(error) = placed {
            if let Err(left) = unlinkat(&dir, &temp, AtFlags::empty()) {
                tracing::warn!(target: "miyu::fs", error = %left, "temporary file left behind");
            }
            return Err(error);
        }
        Ok(())
    }

    /// 在打开了的目录 `dir` 里只许新建、不跟链接地建一个临时文件，名字照 `name` 起，撞上了就换。
    fn temp_in(dir: &OwnedFd, name: &OsStr) -> io::Result<(String, OwnedFd)> {
        let flags =
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mut last: io::Error = Errno::EXIST.into();
        for _ in 0..TRIES {
            let temp = temp_name(name);
            match openat(dir, temp.as_str(), flags, Mode::from_raw_mode(0o666)) {
                Ok(fd) => return Ok((temp, fd)),
                Err(Errno::EXIST) => last = Errno::EXIST.into(),
                Err(error) => return Err(error.into()),
            }
        }
        Err(last)
    }
}

#[cfg(test)]
mod tests;

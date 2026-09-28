//! 路上一层链接都不跟地打开（`docs/blueprint/fs.md` 第四节，施工 5-10 下）：交进来的是换过的真实位置，路上本来没有
//! 链接；有了，就是检查完以后被换过，不开，报 `ELOOP`。
//!
//! Linux 用 `openat2` 带 `RESOLVE_NO_SYMLINKS`，内核没有它、被容器挡掉的退回一层一层打开；macOS 用 `O_NOFOLLOW_ANY`；
//! 别的 Unix 一层一层打开。系统接口经 `rustix` 调，不写 `unsafe`。

use std::fs::File;
use std::io;
use std::os::fd::OwnedFd;
use std::path::Path;

use rustix::fs::{Mode, OFlags};

/// 打开一份要读的文件：只读、不阻塞（FIFO 没人写时不卡住）。
pub(crate) fn open_read(real: &Path) -> io::Result<File> {
    open(real, OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW).map(File::from)
}

/// 打开一个目录，之后相对它看、建、改名、删：写文件时的上级目录。
pub(crate) fn open_dir(real: &Path) -> io::Result<OwnedFd> {
    open(real, dir_flags())
}

/// 目录怎么开：Linux 上只要走得进去（`O_PATH`，不要求能读）。
#[cfg(target_os = "linux")]
fn dir_flags() -> OFlags {
    OFlags::PATH | OFlags::DIRECTORY
}

/// 目录怎么开：只读（`O_PATH` 只有 Linux 有）。
#[cfg(not(target_os = "linux"))]
fn dir_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY
}

#[cfg(target_os = "linux")]
fn open(real: &Path, flags: OFlags) -> io::Result<OwnedFd> {
    use rustix::fs::{CWD, ResolveFlags, openat2};
    use rustix::io::Errno;

    let flags = flags | OFlags::CLOEXEC;
    match openat2(CWD, real, flags, Mode::empty(), ResolveFlags::NO_SYMLINKS) {
        Ok(fd) => Ok(fd),
        // 内核没有 `openat2`（5.6 以前），或者被容器的系统调用过滤挡掉了：一层一层打开，照样挡得住。
        Err(Errno::NOSYS | Errno::PERM) => by_components(real, flags),
        Err(error) => Err(error.into()),
    }
}

/// macOS 上 `O_NOFOLLOW_ANY` 连最后一层也管，和 `O_NOFOLLOW` 一起写报 `EINVAL`：去掉后一个。
#[cfg(target_os = "macos")]
fn open(real: &Path, flags: OFlags) -> io::Result<OwnedFd> {
    let flags = (flags - OFlags::NOFOLLOW) | OFlags::CLOEXEC | OFlags::NOFOLLOW_ANY;
    rustix::fs::open(real, flags, Mode::empty()).map_err(Into::into)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn open(real: &Path, flags: OFlags) -> io::Result<OwnedFd> {
    by_components(real, flags | OFlags::CLOEXEC)
}

/// 一层一层打开：从根目录起，每一层相对上一层打开、不跟链接，最后一层照 `flags` 打开。路上哪一层是链接，报
/// `ELOOP`。`real` 要是绝对的，里面没有 `.`、`..`：换过的真实位置本来就是这样。
#[cfg(not(target_os = "macos"))]
pub(crate) fn by_components(real: &Path, flags: OFlags) -> io::Result<OwnedFd> {
    use std::path::Component;

    let mut names = Vec::new();
    for component in real.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => names.push(name),
            _ => return Err(io::Error::from(io::ErrorKind::InvalidInput)),
        }
    }
    if !real.has_root() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let Some((last, before)) = names.split_last() else {
        return rustix::fs::open("/", flags | OFlags::CLOEXEC, Mode::empty()).map_err(Into::into);
    };
    let mut dir = step(rustix::fs::open("/", step_flags(), Mode::empty()))?;
    for name in before {
        dir = step(rustix::fs::openat(&dir, *name, step_flags(), Mode::empty()))?;
    }
    // 最后一层先不带 `O_DIRECTORY` 开：带着它碰到链接报的是「不是目录」，认不出是链接。查过不是链接，再看是不是目录。
    let wanted_dir = flags.contains(OFlags::DIRECTORY);
    let flags = (flags - OFlags::DIRECTORY) | OFlags::CLOEXEC | OFlags::NOFOLLOW;
    let fd = step(rustix::fs::openat(&dir, *last, flags, Mode::empty()))?;
    if wanted_dir && kind(&fd)? != rustix::fs::FileType::Directory {
        return Err(rustix::io::Errno::NOTDIR.into());
    }
    Ok(fd)
}

/// 路上每一层目录怎么开：不跟链接，只要走得进去（`O_PATH`）。`O_PATH` 碰到链接会开出链接本身，由 [`step`] 认出来。
#[cfg(target_os = "linux")]
fn step_flags() -> OFlags {
    OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC
}

/// 路上每一层目录怎么开：只读，不跟链接，要能读那一层（`O_PATH` 只有 Linux 有）。
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn step_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
}

/// 开出来的是链接本身（Linux 上 `O_PATH` 带 `O_NOFOLLOW` 碰到链接）：当成路上有链接，报 `ELOOP`。
#[cfg(not(target_os = "macos"))]
fn step(opened: rustix::io::Result<OwnedFd>) -> io::Result<OwnedFd> {
    let fd = opened?;
    if kind(&fd)? == rustix::fs::FileType::Symlink {
        return Err(rustix::io::Errno::LOOP.into());
    }
    Ok(fd)
}

/// 开出来的是什么。
#[cfg(not(target_os = "macos"))]
fn kind(fd: &OwnedFd) -> io::Result<rustix::fs::FileType> {
    let stat = rustix::fs::fstat(fd)?;
    Ok(rustix::fs::FileType::from_raw_mode(stat.st_mode))
}

#[cfg(test)]
mod tests;

//! 往本人的数据根里写只有本人和 SYSTEM 读得到的文件（`docs/blueprint/sandbox/windows.md`「怎么走」第 4 条第 7 步）：
//! 装好的记录、提升过的自己没成时写下的原因。
//!
//! 写的一方可能是提升过的管理员，写的地方却是本人管着的数据根，所以：
//! - 先认标记（[`check_root`]）：给的地方不是 Miyu 的数据根，一个字节都不写；
//! - `state`、`state/sandbox` 是链接（符号链接、目录联接）的，不经它写（[`sandbox_dir`]）：不然提前放好的一个联接，
//!   就能让管理员身份的写落到别处去；
//! - 文件只许新建，不跟链接；Windows 上建的时候访问控制就只给本人和 SYSTEM（[`write_private`]）。
//!
//! 挡不住检查完、写之前被换掉：能换的人本来就能改本人的数据根（和 `fs.md` 那一条一样）。

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use miyu_store::root::MARKER;

use super::InstallError;

/// 数据根里有没有标记：只看在不在，文件、目录都算，和建骨架那边一样（`store.md`「怎么走」第 2 条）。
///
/// # Errors
///
/// 没有标记：[`InstallError::NotDataRoot`]；看不了（例如没有权限）：`check owner` 这一步没成，照原话说。
pub(crate) fn check_root(home: &Path) -> Result<(), InstallError> {
    match fs::symlink_metadata(home.join(MARKER)) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Err(InstallError::NotDataRoot {
            path: home.display().to_string(),
        }),
        Err(error) => Err(InstallError::failed("check owner", error)),
    }
}

/// 数据根下的 `state/sandbox/`：一层一层来，每一层没有就建，再查它不是链接，查过了才往下一层走。
///
/// 不能先一口气建好再查：`state` 要是一个链接，一口气建的时候就顺着它把 `sandbox` 建到别处去了。
///
/// # Errors
///
/// 建不了；哪一层是链接或者不是目录。
pub(crate) fn sandbox_dir(home: &Path) -> io::Result<PathBuf> {
    let state = home.join("state");
    plain_dir(&state)?;
    let sandbox = state.join("sandbox");
    plain_dir(&sandbox)?;
    Ok(sandbox)
}

/// `path` 本身是一个真的目录：没有就只建这一层；是链接，或者不是目录的，报错。
///
/// Windows 上标准库把名字代理类的重解析点当链接：符号链接、目录联接都在里面，要挡的就是它们。别的重解析点（例如
/// OneDrive 按需下载的目录）不算链接，照常写：数据根放在同步的目录里，也装得上。
fn plain_dir(path: &Path) -> io::Result<()> {
    match fs::create_dir(path) {
        Err(error) if error.kind() != io::ErrorKind::AlreadyExists => return Err(error),
        _ => {}
    }
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other(format!(
            "{} is a link: not writing through it",
            path.display()
        )));
    }
    if !metadata.is_dir() {
        return Err(io::Error::other(format!(
            "{} is not a directory",
            path.display()
        )));
    }
    Ok(())
}

/// 只许新建地建 `path`，写进 `bytes`，同步。已经有了的（连同链接）不覆盖，不跟链接。Windows 上建的时候访问控制就是
/// `D:P(A;;FA;;;<sid>)(A;;FA;;;SY)`；别的平台上（只有测试走这里）是 0600。
///
/// # Errors
///
/// 已经有了；建不了、写不了。
pub(crate) fn write_private(path: &Path, bytes: &[u8], sid: &str) -> io::Result<()> {
    let mut file = create_private(path, sid)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Windows：建的时候访问控制就只给本人和 SYSTEM。
#[cfg(windows)]
fn create_private(path: &Path, sid: &str) -> io::Result<fs::File> {
    super::winsys::create_private(path, sid)
}

/// 别的平台：只有测试走这里，0600，只许新建（`O_EXCL` 不跟链接）。
#[cfg(unix)]
fn create_private(path: &Path, _sid: &str) -> io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
}

/// 删掉 `path`，本来没有不算错。
///
/// # Errors
///
/// 删不了。
pub(crate) fn remove_if_there(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// 写一份只给本人和 SYSTEM 的文件 `name`，放进 `state/sandbox/`：先删上次留下的临时文件，只许新建地写临时文件，同步，
/// 再改名盖掉旧的。改名是同一个目录里的，要么是旧的、要么是新的，不会只有半份。
///
/// # Errors
///
/// 哪一步读写不了、目录是链接：交回系统的原话。
pub(crate) fn replace_private(home: &Path, name: &str, bytes: &[u8], sid: &str) -> io::Result<()> {
    let dir = sandbox_dir(home)?;
    let temporary = dir.join(format!("{name}.tmp"));
    remove_if_there(&temporary)?;
    write_private(&temporary, bytes, sid)?;
    fs::rename(&temporary, dir.join(name))
}

#[cfg(test)]
mod tests;

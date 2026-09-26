//! 落盘的两件小事（`docs/designs/07-存储.md` 第四节「各平台的坑」）：同步目录；建目录时，新建的
//! 每一层都同步它的上一层。新建文件、新建目录、改名以后，上一层不同步，断电以后这一项可能没了。

use std::fs;
use std::io;
use std::path::Path;

/// 建目录，连同缺的上层。新建的每一层都同步它的上一层，建目录这件事本身才算落盘；已经有的不动。
///
/// Unix 上新建的权限 0700，只有本人能进；已经有的不改：数据根可能是人自己建、自己设的，权限
/// 不对由 `miyu doctor` 报告（`22-命令行.md` 第五节）。Windows 上靠用户目录本身的访问控制。
///
/// # Errors
///
/// 建不了；该是目录的地方是个文件；同步不了。
pub(crate) fn create_dir(dir: &Path) -> io::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    let parent = dir.parent().filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        create_dir(parent)?;
    }
    #[cfg(unix)]
    let created = {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(dir)
    };
    #[cfg(not(unix))]
    let created = fs::create_dir(dir);
    match created {
        Ok(()) => {}
        // 同一时刻别处建好的，也算。
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists && dir.is_dir() => {
            return Ok(());
        }
        Err(error) => return Err(error),
    }
    match parent {
        Some(parent) => sync_dir(parent),
        None => Ok(()),
    }
}

/// 同步目录：新建文件、新建目录、改名以后做，这一项本身才算落盘。
///
/// # Errors
///
/// 打不开这个目录，或者同步不了。
#[cfg(unix)]
pub(crate) fn sync_dir(dir: &Path) -> io::Result<()> {
    fs::File::open(dir)?.sync_all()
}

/// Windows 上不用同步目录，也打不开目录来同步。
#[cfg(not(unix))]
pub(crate) fn sync_dir(_dir: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests;

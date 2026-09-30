//! 落盘的几件小事（`docs/designs/07-存储.md` 第四节「各平台的坑」）：同步目录；建目录时，新建的
//! 每一层都同步它的上一层。新建文件、新建目录、改名以后，上一层不同步，断电以后这一项可能没了。
//! 先写临时文件再改名的，临时文件怎么建、用不上了怎么删（blob、生成的文件共用，施工 8-1 从 `blob.rs` 挪来）。

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

/// 临时文件的名字最多换几次：撞上的都是崩溃留下的，换几次总能换开。
const TEMP_TRIES: u32 = 64;

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

/// 在 `dir` 里新建一个临时文件，名字照 `name` 起，只许新建。撞上崩溃留下的同名文件，换下一个名字。
///
/// # Errors
///
/// 建不了；一连 64 个名字都被占了。
pub(crate) fn create_temp(
    dir: &Path,
    mut name: impl FnMut() -> String,
) -> io::Result<(PathBuf, File)> {
    for _ in 0..TEMP_TRIES {
        let path = dir.join(name());
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "{TEMP_TRIES} temporary file names in a row are taken in {}",
            dir.display()
        ),
    ))
}

/// 删掉用不上的临时文件。
#[expect(
    clippy::let_underscore_must_use,
    reason = "删不掉就留着，不耽误这一次：blob 的由回收清，生成的文件的留在旁边"
)]
pub(crate) fn discard(temp: &Path) {
    let _ = fs::remove_file(temp);
}

#[cfg(test)]
mod tests;

//! blob（`docs/designs/07-存储.md` 第五节）：图片、文件、超长的工具输出、策略快照这些大内容，按
//! 内容哈希存成单独的文件，事件里只放引用（`03-事件模型.md` E4）。一个账号一份，不跨账号去重
//! （S5）：`home/<账号>/blobs/<前两位>/<64 位十六进制>`。
//!
//! 存：先写进 `tmp/` 里的临时文件、同步，再改名成它的哈希、同步目录。返回时它已经落了盘，这才能
//! 写引用它的事件（07 第四节「先落 blob，再写引用它的事件」）。改名是原子的，所以磁盘上不会有
//! 写了一半的 blob；崩溃留在 `tmp/` 里的，回收的时候再清。

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use miyu_kernel::id::ContentHash;

use crate::durable::{create_dir, sync_dir};

/// 放临时文件的目录。和前两位的目录（两位十六进制）撞不上。
const TMP: &str = "tmp";

/// 临时文件的名字最多换几次：撞上的都是崩溃留下的，换几次总能换开。
const TEMP_TRIES: u32 = 64;

/// 一个账号的 blob。
#[derive(Debug, Clone)]
pub struct Blobs {
    /// `home/<账号>/blobs/`。
    dir: PathBuf,
}

impl Blobs {
    /// 一个账号的 blob 目录，平时是 [`DataRoot::blobs`](crate::root::DataRoot::blobs)。用到时才建。
    pub fn new(dir: PathBuf) -> Blobs {
        Blobs { dir }
    }

    /// 这个哈希的 blob 放在哪：`<前两位>/<64 位>`。
    pub fn path(&self, hash: &ContentHash) -> PathBuf {
        self.fan(hash).join(hash.hex())
    }

    /// 存一份内容，返回它的哈希。返回时它已经落了盘。已经有了的不重写，只把修改时间刷成现在：
    /// 回收的宽限期按修改时间算，刚又传了一遍的不能当成旧的删掉（07 第五节）。
    ///
    /// # Errors
    ///
    /// 建不了目录、写不进、同步不了、改不了名。
    pub fn put(&self, content: &[u8]) -> io::Result<ContentHash> {
        let hash = ContentHash::of(content);
        let fan = self.fan(&hash);
        let path = fan.join(hash.hex());
        if path.is_file() {
            freshen(&path)?;
            // 它可能是同一时刻别处刚改好名、还没同步目录的。
            sync_dir(&fan)?;
            return Ok(hash);
        }
        let tmp = self.dir.join(TMP);
        create_dir(&tmp)?;
        let (temp, file) = create_temp(&tmp, temp_name)?;
        let stored = store(file, content, &temp, &fan, &path);
        if stored.is_err() {
            discard(&temp);
        }
        stored?;
        Ok(hash)
    }

    /// 取一份内容，核对它的哈希：读出来和名字对不上，报错，不悄悄用，也不删（07 第五节）。
    ///
    /// # Errors
    ///
    /// 没有这个 blob；读出来和名字对不上；读不了。
    pub fn get(&self, hash: &ContentHash) -> Result<Vec<u8>, BlobError> {
        let content = match fs::read(self.path(hash)) {
            Ok(content) => content,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(BlobError::Missing(hash.clone()));
            }
            Err(error) => return Err(BlobError::Io(error)),
        };
        if ContentHash::of(&content) != *hash {
            return Err(BlobError::Corrupt(hash.clone()));
        }
        Ok(content)
    }

    /// 前两位的目录。
    fn fan(&self, hash: &ContentHash) -> PathBuf {
        let hex = hash.hex();
        self.dir.join(hex.get(..2).unwrap_or(hex))
    }
}

/// 取不出来。
#[derive(Debug)]
pub enum BlobError {
    /// 没有这个 blob。
    Missing(ContentHash),
    /// 读出来的内容和它的名字对不上：磁盘坏了，或者被别的程序改过。不自动修，也不删。
    Corrupt(ContentHash),
    /// 读不了。
    Io(io::Error),
}

impl fmt::Display for BlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlobError::Missing(hash) => write!(f, "没有 blob {hash}"),
            BlobError::Corrupt(hash) => {
                write!(f, "blob {hash} 读出来的内容和它的名字对不上，不动它")
            }
            BlobError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for BlobError {}

/// 写进临时文件、同步、关上（Windows 上开着的文件改不了名），改名成它的哈希，再同步目录。
fn store(mut file: File, content: &[u8], temp: &Path, fan: &Path, path: &Path) -> io::Result<()> {
    file.write_all(content)?;
    file.sync_data()?;
    drop(file);
    create_dir(fan)?;
    settle(temp, path)?;
    sync_dir(fan)
}

/// 改名成它的哈希。改名失败、可目标已经有了，算成功：两个会话同时存同一份，Windows 上后改名的
/// 那个可能失败，内容是一样的，删掉自己的临时文件就行。
fn settle(temp: &Path, path: &Path) -> io::Result<()> {
    match fs::rename(temp, path) {
        Ok(()) => Ok(()),
        Err(_) if path.is_file() => {
            discard(temp);
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// 新建一个临时文件，只许新建。撞上崩溃留下的同名文件，换下一个名字。
fn create_temp(dir: &Path, mut name: impl FnMut() -> String) -> io::Result<(PathBuf, File)> {
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
            "{} 里的临时文件名一连 {TEMP_TRIES} 个都被占了",
            dir.display()
        ),
    ))
}

/// 临时文件的名字：进程号加一个计数。
fn temp_name() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// 把修改时间刷成现在。
fn freshen(path: &Path) -> io::Result<()> {
    OpenOptions::new()
        .write(true)
        .open(path)?
        .set_modified(SystemTime::now())
}

/// 删掉用不上的临时文件。
#[expect(
    clippy::let_underscore_must_use,
    reason = "删不掉就留在 tmp/ 里，回收的时候再清，不耽误这一次"
)]
fn discard(temp: &Path) {
    let _ = fs::remove_file(temp);
}

#[cfg(test)]
mod tests;

//! 一条后台命令的输出文件（`docs/blueprint/store.md` 的 `jobs/<编号>.out`）：读的线程一段段写进去，不截；报了结束就关上，
//! 之后读到的不再写（还拿着管道的孙进程）。结束时整份存成 blob，数出有多少个字。

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use miyu_kernel::id::ContentHash;
use miyu_store::blob::Blobs;

use crate::TARGET;

/// 一条后台命令的输出文件。
pub(super) struct Output {
    path: PathBuf,
    /// 写的一头；关上了、写不进去了的是空的。
    file: Mutex<Option<File>>,
}

impl Output {
    /// 在 `path` 上开好的 `file`。
    pub(super) fn new(path: PathBuf, file: File) -> Output {
        Output {
            path,
            file: Mutex::new(Some(file)),
        }
    }

    /// 写一段。关上了的不写；写不进去的（磁盘满了之类）记一行，关上，之后的都不写：命令照跑，读的线程照读，不让它
    /// 卡在写满的管道上。
    pub(super) fn write(&self, text: &str) {
        let mut file = self.lock();
        let Some(open) = file.as_mut() else {
            return;
        };
        if let Err(error) = open.write_all(text.as_bytes()) {
            tracing::warn!(target: TARGET, error = %error, "job output not written");
            *file = None;
        }
    }

    /// 关上：之后读到的不再写。
    pub(super) fn close(&self) {
        *self.lock() = None;
    }

    /// 整份存成 blob，交回它的哈希和有多少个字（Unicode 字符）。读不出来、存不下来的记一行，两样都没有：回报里不写
    /// 输出，她也就不会去读一份不在的。碰磁盘，在阻塞线程里调。
    pub(super) fn stored(&self, blobs: &Blobs) -> (Option<ContentHash>, Option<u64>) {
        let stored = std::fs::read(&self.path).and_then(|bytes| {
            let hash = blobs.put(&bytes)?;
            Ok((hash, chars(&bytes)))
        });
        match stored {
            Ok((hash, chars)) => (Some(hash), Some(chars)),
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "job output not stored");
                (None, None)
            }
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<File>> {
        self.file.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// 有多少个字：写进去的都是解好的 UTF-8，不是接续字节的每个字节算一个字，和前台的数法一样。
fn chars(bytes: &[u8]) -> u64 {
    bytes.iter().filter(|&&byte| byte & 0xC0 != 0x80).count() as u64
}

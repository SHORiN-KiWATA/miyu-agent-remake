//! 单实例锁（`docs/designs/07-存储.md` 第十节）：一个数据根上只跑一个核心。
//!
//! 锁在 `run/core.lock` 上，用标准库的文件锁，三个平台一样。一直拿到核心退出：进程没了，操作系统
//! 替它放开，崩了也不会留下一把死锁。同一个进程里打开两次也是两把，第二把拿不到。

use std::fs::{File, OpenOptions, TryLockError};

use miyu_store::root::DataRoot;

use crate::error::OpenError;

/// 锁文件的名字，在 `run/` 里。
const FILE: &str = "core.lock";

/// 拿着的单实例锁：丢掉它就放开。
#[derive(Debug)]
pub struct Lock {
    /// 锁在这个打开的文件上：文件关了，锁就放开。
    _file: File,
}

impl Lock {
    /// 拿锁，不等：拿不到就是已经有一个核心在跑。
    ///
    /// # Errors
    ///
    /// 拿不到；打不开锁文件。
    pub fn acquire(root: &DataRoot) -> Result<Lock, OpenError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.run().join(FILE))?;
        match file.try_lock() {
            Ok(()) => Ok(Lock { _file: file }),
            Err(TryLockError::WouldBlock) => Err(OpenError::Running),
            Err(TryLockError::Error(error)) => Err(OpenError::Io(error)),
        }
    }
}

#[cfg(test)]
mod tests;

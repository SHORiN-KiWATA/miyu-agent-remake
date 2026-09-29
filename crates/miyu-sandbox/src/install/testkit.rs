//! 装沙盒的几份测试共用的：用完就删的临时目录，带不带数据根的标记；替谁装。
//!
//! Windows 上写出来的文件只给「本人」和 SYSTEM：测试里的本人得是跑测试的这个用户，不然自己写的文件自己读不回来、
//! 删不掉。所以 Windows 上用当前用户的 SID，别的平台上随便一个写法对的。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_store::root::MARKER;

use super::Owner;

/// 一个用完就删的临时目录。
pub(crate) struct Dir(PathBuf);

impl Dir {
    /// 空的，不是数据根。
    pub(crate) fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-install-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建得了目录");
        Dir(dir)
    }

    /// 有数据根的标记。
    pub(crate) fn root() -> Dir {
        let dir = Dir::new();
        std::fs::write(dir.0.join(MARKER), "test\n").expect("写得进标记");
        dir
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// 替这个目录的主人装：数据根是它，SID 是 [`sid`]。
    pub(crate) fn owner(&self) -> Owner {
        Owner::new(self.0.clone(), sid()).expect("写法对")
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("临时目录没删掉：{error}");
        }
    }
}

/// 测试里的本人：Windows 上是跑测试的这个用户，别的平台上是一个写法对的。
pub(crate) fn sid() -> String {
    #[cfg(windows)]
    {
        miyu_pipe::current_user().expect("取得到当前用户")
    }
    #[cfg(not(windows))]
    {
        "S-1-5-21-1-2-3-1001".to_string()
    }
}

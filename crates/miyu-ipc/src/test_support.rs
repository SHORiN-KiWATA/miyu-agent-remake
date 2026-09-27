//! 测试用的：临时目录、数据根。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_store::env::{Env, Platform};
use miyu_store::root::DataRoot;

/// 测试用的临时目录：进程号加序号，建好了，用完删掉。
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-ipc-test-{}-{n}", std::process::id()));
        fs::create_dir_all(&dir).expect("临时目录建得了");
        Scratch(dir)
    }

    /// 这个临时目录的路径。
    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// 里面的一个数据根，建好了骨架。
    pub(crate) fn root(&self) -> DataRoot {
        let root = root_at(&self.0.join("root"));
        root.prepare().expect("临时目录里建得了骨架");
        root
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// 在 `path` 的数据根，不碰磁盘。
pub(crate) fn root_at(path: &Path) -> DataRoot {
    DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(path.as_os_str().to_owned()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .expect("MIYU_HOME 是绝对路径")
}

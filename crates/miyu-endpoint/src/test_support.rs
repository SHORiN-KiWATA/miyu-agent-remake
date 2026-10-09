//! 端点自己的单元测试共用的：临时的数据根、出厂的资源目录。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_store::env::{Env, Platform};
use miyu_store::root::DataRoot;

/// 一个临时的数据根，建好了骨架；目录名带 `name`、进程号和序号，几个测试不撞。交回数据根和它的目录。
pub(crate) fn temp_root(name: &str) -> (DataRoot, PathBuf) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("miyu-endpoint-{name}-{}-{n}", std::process::id()));
    let root = DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(dir.clone().into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("建得了骨架");
    (root, dir)
}

/// 源码树里出厂的资源目录。
pub(crate) fn resources() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

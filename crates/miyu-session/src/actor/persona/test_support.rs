//! 换快照的测试共用的（施工 P-2 下从 `tests.rs` 挪出来）：一个临时数据根，Miyu 住在管理员 alice 的家目录。

use std::path::PathBuf;

use miyu_store::env::{Env, Platform};
use miyu_store::root::DataRoot;

/// 临时目录：用完删掉。
pub(super) struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}

/// 一个临时数据根，Miyu 的人设是 `persona`。
pub(super) fn scratch_root(name: &str, persona: &str) -> (Scratch, DataRoot) {
    let scratch =
        Scratch(std::env::temp_dir().join(format!("miyu-persona-{name}-{}", std::process::id())));
    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.0.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("临时目录里建得了骨架");
    write(&root, "persona.md", persona);
    (scratch, root)
}

/// 写 Miyu 的一份字。
pub(super) fn write(root: &DataRoot, file: &str, text: &str) {
    let dir = root.path().join("home/alice/personas/miyu/prompts");
    std::fs::create_dir_all(&dir).expect("建得了人格目录");
    std::fs::write(dir.join(file), text).expect("写得进");
}

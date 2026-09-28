//! 测试用的（施工 5-4 上）：cargo 编出来的助手在哪。会话、命令行的测试要真的经助手起命令，在各自的 dev-dependencies
//! 里打开 `testkit`。

use std::path::{Path, PathBuf};

use crate::HELPER;

/// cargo 编出来的助手：`target/<profile>/miyu-sandbox`，和测试程序所在的 `deps/` 同一层。`cargo test --workspace`
/// 编了它；只测一个包时，先 `cargo build -p miyu-sandbox`。
///
/// # Panics
///
/// 不知道测试程序在哪，或者它不在 `target/<profile>/deps/` 里；那里没有助手。
pub fn built_helper() -> PathBuf {
    let exe = std::env::current_exe().expect("知道测试程序在哪");
    let dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("测试程序在 target/<profile>/deps/ 里");
    let helper = dir.join(HELPER);
    assert!(
        helper.is_file(),
        "{} 不在：先 cargo build -p miyu-sandbox（cargo test --workspace 会编它）",
        helper.display()
    );
    helper
}

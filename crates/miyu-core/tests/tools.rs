//! 核心起来时登记基础系统（施工 4-4 上）：工具目录里有读的三件（施工 4-4 下）、写的三件（施工 4-6）、`shell`（施工 4-8）、`history`（施工 6-4）、`subagent`（施工 7-5，7-5 再补改名）、`jobs`（施工 7-4）、`send_message`（施工 7-7，施工 C-5 从 `message_agent` 改名）、`sessions`（施工 C-3）、`session_usage`（施工 8-15）、`ask_user`（施工 D-2）和 `todowrite`（施工 D-3），记忆这个软件包的 `remember`、`forget`、`memory_search`（施工 R-3 中）；资源目录坏了，
//! 说是哪一份。

use std::path::Path;

use miyu_store::resources::ResourceRoot;

#[test]
fn the_catalog_has_the_base_system() {
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let catalog = miyu_core::tools(&resources).expect("出厂的资源读得出来");
    let names: Vec<&str> = catalog.specs().map(|spec| spec.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "ask_user",
            "edit",
            "forget",
            "glob",
            "grep",
            "history",
            "jobs",
            "memory_search",
            "read",
            "remember",
            "send_message",
            "session_usage",
            "sessions",
            "shell",
            "subagent",
            "todowrite",
            "trash",
            "write"
        ]
    );
}

#[test]
fn broken_resources_say_which_file() {
    let resources = ResourceRoot::at(std::env::temp_dir().join("miyu-core-no-resources-here"));
    let error = miyu_core::tools(&resources).expect_err("读不出来");
    // 先读的是几件工具共用的那几句。
    assert!(error.contains("missing.txt"), "{error}");
}

//! 核心起来时登记基础系统（施工 4-4 上）：工具目录里有读的三件（施工 4-4 下）和写的 `write`、`edit`（施工 4-6 上、中）；资源目录坏了，
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
        ["edit", "glob", "grep", "read", "shell", "trash", "write"]
    );
}

#[test]
fn broken_resources_say_which_file() {
    let resources = ResourceRoot::at(std::env::temp_dir().join("miyu-core-no-resources-here"));
    let error = miyu_core::tools(&resources).expect_err("读不出来");
    // 先读的是几件工具共用的那几句。
    assert!(error.contains("missing.txt"), "{error}");
}

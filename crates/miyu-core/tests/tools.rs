//! 核心起来时登记基础系统（施工 4-4 上）：工具目录里有读的三件（施工 4-4 下）、写的三件（施工 4-6）、`shell`（施工 4-8）、`history`（施工 6-4）、`subagent`（施工 7-5，7-5 再补改名）、`jobs`（施工 7-4）、`send_message`（施工 7-7，施工 C-5 从 `message_agent` 改名）、`sessions`（施工 C-3）、`session_usage`（施工 8-15）、`ask_user`（施工 D-2）和 `todowrite`（施工 D-3），记忆这个软件包的 `remember`、`forget`、`memory_search`（施工 R-3 中）；资源目录坏了，
//! 说是哪一份。

use std::path::Path;

use miyu_store::packages::{Found, Packages};
use miyu_store::resources::ResourceRoot;

/// 仓库的资源目录。
fn shipped_resources() -> ResourceRoot {
    ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

/// 出厂的那一层清单，去掉 `without` 这几个包（施工 F-2：没装的就是没有清单）。
fn installed(resources: &ResourceRoot, without: &[&str]) -> Vec<Found> {
    let mut found = Packages::shipped(resources).read();
    found.retain(|one| !without.contains(&one.id.as_str()));
    found
}

/// 工具目录里的名字，照名字排。
fn names(found: &[Found]) -> Vec<String> {
    let resources = shipped_resources();
    let catalog = miyu_core::tools(&resources, found).expect("出厂的资源读得出来");
    catalog.specs().map(|spec| spec.name.clone()).collect()
}

#[test]
fn the_catalog_has_the_base_system() {
    let resources = shipped_resources();
    let catalog =
        miyu_core::tools(&resources, &installed(&resources, &[])).expect("出厂的资源读得出来");
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

/// 内置包照清单启用（施工 F-2，设计 30 第二节第 3 条）：没有清单的，它的工具不登记。
#[test]
fn a_builtin_without_its_manifest_brings_no_tools() {
    let resources = shipped_resources();
    let names_without = |without: &[&str]| names(&installed(&resources, without));
    let without_memory = names_without(&["memory"]);
    assert!(without_memory.contains(&"read".to_string()));
    for tool in ["remember", "forget", "memory_search"] {
        assert!(
            !without_memory.contains(&tool.to_string()),
            "没装记忆，没有 {tool}"
        );
    }
    assert_eq!(
        names_without(&["basesystem"]),
        ["forget", "memory_search", "remember"],
        "没装基础系统，只有记忆的三件"
    );
    assert!(names_without(&["basesystem", "memory"]).is_empty());
}

/// 写错了的清单不算装了（施工 F-2）：照读成了的认。
#[test]
fn a_broken_manifest_does_not_count_as_installed() {
    let resources = shipped_resources();
    let mut found = installed(&resources, &[]);
    for one in &mut found {
        if one.id == "memory" {
            one.read = Err(miyu_store::packages::Issue::Unreadable(
                std::io::Error::other("坏了"),
            ));
        }
    }
    assert!(!names(&found).contains(&"remember".to_string()));
}

/// 出厂的内置包清单，这一份核心都编进来了；编进来的也都有出厂的清单（施工 F-2，默认的 cargo 开关）。
#[test]
fn every_shipped_builtin_is_compiled_in() {
    let resources = shipped_resources();
    let mut shipped: Vec<String> = installed(&resources, &[])
        .into_iter()
        .filter(|one| {
            one.read
                .as_ref()
                .is_ok_and(|manifest| manifest.kind == miyu_config::package::PackageKind::Builtin)
        })
        .map(|one| one.id)
        .collect();
    shipped.sort();
    let mut built_in: Vec<String> = miyu_core::built_in()
        .iter()
        .map(ToString::to_string)
        .collect();
    built_in.sort();
    assert_eq!(shipped, built_in);
}

#[test]
fn broken_resources_say_which_file() {
    let resources = ResourceRoot::at(std::env::temp_dir().join("miyu-core-no-resources-here"));
    let found = installed(&shipped_resources(), &[]);
    let error = miyu_core::tools(&resources, &found).expect_err("读不出来");
    // 先读的是几件工具共用的那几句。
    assert!(error.contains("missing.txt"), "{error}");
}

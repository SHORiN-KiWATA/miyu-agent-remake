//! 装了的功能（施工 F-3 上）：工具归哪个功能；以前的快照记的包编号照现在的功能读。

use super::{Feature, Features};

fn feature(id: &str, package: &str, tools: &[&str]) -> Feature {
    Feature {
        id: id.to_string(),
        package: package.to_string(),
        tools: tools.iter().map(ToString::to_string).collect(),
    }
}

/// 基础系统两个功能、人格记忆一个没写工具、接入QQ 一个没写工具。
fn installed() -> Features {
    Features::new(vec![
        feature("files", "basesystem", &["read", "write"]),
        feature("commands", "basesystem", &["shell"]),
        feature("memory", "memory", &[]),
        feature("qq", "onebot", &[]),
    ])
}

#[test]
fn a_tool_belongs_to_the_feature_that_lists_it() {
    let features = installed();
    assert_eq!(features.of_tool("basesystem", "write"), Some("files"));
    assert_eq!(features.of_tool("basesystem", "shell"), Some("commands"));
    assert_eq!(
        features.of_tool("basesystem", "jobs"),
        None,
        "几个功能都没列它"
    );
}

#[test]
fn a_package_with_one_feature_owns_all_its_tools() {
    let features = installed();
    assert_eq!(features.of_tool("memory", "remember"), Some("memory"));
    assert_eq!(features.of_tool("onebot", "qq_send"), Some("qq"));
    assert_eq!(features.of_tool("nowhere", "x"), None, "没装的包");
}

#[test]
fn a_listed_tool_wins_even_in_a_package_with_one_feature() {
    let features = Features::new(vec![feature("only", "p", &["a"])]);
    assert_eq!(features.of_tool("p", "a"), Some("only"));
    assert_eq!(features.of_tool("p", "b"), Some("only"));
}

#[test]
fn installed_means_listed() {
    let features = installed();
    assert!(features.installed("qq"));
    assert!(!features.installed("onebot"), "包的编号不是功能");
    assert!(!features.installed("roleplay"));
    let ids: Vec<&str> = features.iter().map(|feature| feature.id.as_str()).collect();
    assert_eq!(ids, ["files", "commands", "memory", "qq"]);
}

#[test]
fn old_package_ids_read_as_their_features() {
    let features = installed();
    let old = ["basesystem", "memory", "onebot", "roleplay"].map(String::from);
    let read: Vec<String> = features.read_legacy(&old).into_iter().collect();
    assert_eq!(
        read,
        ["commands", "files", "memory", "qq", "roleplay"],
        "包换成它的功能；功能编号照旧；不认识的照旧"
    );
}

/// 现在的快照记的是功能的编号：哪怕有个包恰好叫这个名字，也不换成那个包的功能（施工 F-3 上）。
#[test]
fn a_feature_id_is_not_read_as_a_package_of_the_same_name() {
    let features = Features::new(vec![
        feature("files", "basesystem", &["read"]),
        feature("sync", "files", &[]),
    ]);
    let read: Vec<String> = features
        .read_legacy(&["files".to_string()])
        .into_iter()
        .collect();
    assert_eq!(read, ["files"]);
}

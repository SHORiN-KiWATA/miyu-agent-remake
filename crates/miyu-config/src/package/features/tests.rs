//! 包带的功能（施工 F-1，设计 30 第三节）：写了的照写的读；没写的整个包算一个；写了空表的一个都没有；每一种写错报哪个代码、
//! 第几行。

use crate::package::{Code, PackageKind, read};

/// 基础系统那种：内置、必需、两个功能。
const BASESYSTEM: &str = r#"[package]
required = true
protocol = [1, 1]
name = { en = "Base system", zh = "基础系统" }

[features.files]
name = { en = "Files", zh = "文件读写" }
summary = { en = "Read, write and search files" }
tools = ["read", "write", "edit"]

[features.commands]
name = { en = "Commands", zh = "运行命令" }
tools = ["shell"]

[builtin]
"#;

/// 只写了包、没写功能的扩展。
const BRIDGE: &str = r#"[package]
protocol = [1, 1]
name = { en = "Connect QQ", zh = "接入QQ" }
summary = { en = "Talk with the AI on QQ" }

[command]
name = "onebot"
program = "miyu-onebot"
about = { en = "QQ" }

[process]
args = ["serve"]
"#;

/// 读错了的：代码、第几行。
fn wrong(text: &str) -> (Code, Option<usize>) {
    let problem = read(text).expect_err("应该读不成");
    (problem.code, problem.line)
}

#[test]
fn a_builtin_package_reads_its_features_in_order() {
    let manifest = read(BASESYSTEM).unwrap();
    assert_eq!(manifest.kind, PackageKind::Builtin);
    assert!(manifest.required);
    let features = manifest.features_of("basesystem");
    let ids: Vec<&str> = features.iter().map(|feature| feature.id.as_str()).collect();
    assert_eq!(ids, ["files", "commands"], "照写的先后");
    assert_eq!(
        features[0].name.get("zh").map(String::as_str),
        Some("文件读写")
    );
    assert_eq!(features[0].summary.len(), 1);
    assert_eq!(features[0].tools, ["read", "write", "edit"]);
    assert!(features[1].summary.is_empty(), "说明可以不写");
    assert_eq!(features[1].tools, ["shell"]);
    assert_eq!(features[0].line, Some(6), "编号在第几行");
}

#[test]
fn a_package_without_features_is_one_feature_named_after_it() {
    let manifest = read(BRIDGE).unwrap();
    assert_eq!(manifest.features, None);
    let features = manifest.features_of("onebot");
    assert_eq!(features.len(), 1);
    assert_eq!(features[0].id, "onebot");
    assert_eq!(features[0].name, manifest.name);
    assert_eq!(features[0].summary, manifest.summary);
    assert!(features[0].tools.is_empty(), "工具都归它，不用一件件写");
    assert_eq!(features[0].line, None);
}

#[test]
fn an_empty_features_table_means_no_feature_at_all() {
    let manifest = read(&format!("{BRIDGE}\n[features]\n")).unwrap();
    assert_eq!(manifest.features, Some(Vec::new()));
    assert!(manifest.features_of("onebot").is_empty());
}

#[test]
fn interfaces_and_workers_bring_no_feature() {
    let ui = "[package]\nprotocol = [1, 1]\nname = { en = \"Web\" }\n\n[ui]\n";
    assert!(read(ui).unwrap().features_of("web").is_empty());
    let worker = "[package]\nprotocol = [1, 1]\nname = { en = \"Embed\" }\n\n[worker]\nprogram = \"miyu-embed\"\n";
    assert!(read(worker).unwrap().features_of("embed").is_empty());
}

#[test]
fn a_feature_id_is_written_like_a_package_id() {
    let text = BASESYSTEM.replace("[features.commands]", "[features.Commands]");
    assert_eq!(wrong(&text), (Code::BadFeature, Some(11)));
    let text = BASESYSTEM.replace("[features.commands]", "[features.\"9lives\"]");
    assert_eq!(wrong(&text).0, Code::BadFeature);
}

#[test]
fn a_feature_must_be_a_table_with_a_name() {
    let text = BASESYSTEM.replace(
        "[features.commands]\nname = { en = \"Commands\", zh = \"运行命令\" }\ntools = [\"shell\"]\n",
        "[features]\ncommands = 1\n",
    );
    assert_eq!(wrong(&text).0, Code::NotATable);
    let text = BASESYSTEM.replace("name = { en = \"Commands\", zh = \"运行命令\" }\n", "");
    assert_eq!(wrong(&text), (Code::MissingKey, Some(11)), "报在功能那一行");
    let text = BASESYSTEM.replace("tools = [\"shell\"]", "tools = [\"shell\"]\nicon = \"x\"");
    assert_eq!(wrong(&text), (Code::UnknownKey, Some(14)));
    let text = BASESYSTEM.replace(
        "summary = { en = \"Read, write and search files\" }",
        "summary = 3",
    );
    assert_eq!(wrong(&text).0, Code::NotPhrases);
}

#[test]
fn tools_are_tool_names_listed_once_in_the_whole_package() {
    let text = BASESYSTEM.replace("tools = [\"shell\"]", "tools = \"shell\"");
    assert_eq!(wrong(&text), (Code::NotTexts, Some(13)));
    let text = BASESYSTEM.replace("tools = [\"shell\"]", "tools = [\"run shell\"]");
    assert_eq!(wrong(&text), (Code::BadTool, Some(13)));
    let text = BASESYSTEM.replace("tools = [\"shell\"]", "tools = [\"shell\", \"read\"]");
    let problem = read(&text).expect_err("read 列了两次");
    assert_eq!((problem.code, problem.line), (Code::BadTool, Some(13)));
    assert_eq!(problem.detail, "read");
    let text = BASESYSTEM.replace("tools = [\"shell\"]", "tools = [\"shell\", \"shell\"]");
    assert_eq!(
        wrong(&text),
        (Code::BadTool, Some(13)),
        "同一个功能里也只列一次"
    );
    let long = "x".repeat(65);
    let text = BASESYSTEM.replace("tools = [\"shell\"]", &format!("tools = [\"{long}\"]"));
    assert_eq!(wrong(&text).0, Code::BadTool, "最长 64 个");
}

#[test]
fn only_builtin_and_process_packages_write_features() {
    let ui = "[package]\nprotocol = [1, 1]\nname = { en = \"Web\" }\n\n[features]\n\n[ui]\n";
    assert_eq!(wrong(ui), (Code::WrongKind, Some(5)));
    let worker = "[package]\nprotocol = [1, 1]\nname = { en = \"Embed\" }\n\n[worker]\nprogram = \"miyu-embed\"\n\n[features.x]\nname = { en = \"X\" }\n";
    assert_eq!(wrong(worker).0, Code::WrongKind);
}

//! 种类多的两种、必需、平台接入、依赖、小程序（施工 F-1，设计 30 第二节、第五节、第六节）：读成什么；写在不给的种类里、写错
//! 了报哪个代码、第几行。

use crate::package::{Code, Connection, PackageKind, Worker, read};

/// 内置模型那种小程序。
const EMBED: &str = r#"[package]
kind = "worker"
protocol = [1, 1]
name = { en = "Built-in model", zh = "内置模型" }

[worker]
program = "miyu-embed"
args = ["serve"]
"#;

/// 人格记忆那种：内置、推荐内置模型。
const MEMORY: &str = r#"[package]
kind = "builtin"
protocol = [1, 1]
name = { en = "Memory", zh = "人格记忆" }

[recommends]
workers = ["embed"]
"#;

/// 接入QQ 那种：扩展、平台接入。
const BRIDGE: &str = r#"[package]
kind = "process"
protocol = [1, 1]
name = { en = "Connect QQ" }

[command]
name = "onebot"
program = "miyu-onebot"
about = { en = "QQ" }

[process]
args = ["serve"]

[connection]
platform = "qq"
"#;

/// 读错了的：代码、第几行。
fn wrong(text: &str) -> (Code, Option<usize>) {
    let problem = read(text).expect_err("应该读不成");
    (problem.code, problem.line)
}

#[test]
fn a_worker_reads_its_program_and_args() {
    let manifest = read(EMBED).unwrap();
    assert_eq!(manifest.kind, PackageKind::Worker);
    assert_eq!(
        manifest.worker,
        Some(Worker {
            program: "miyu-embed".to_string(),
            args: vec!["serve".to_string()],
        })
    );
    let bare = read(&EMBED.replace("args = [\"serve\"]\n", "")).unwrap();
    assert!(bare.worker.unwrap().args.is_empty(), "参数可以不写");
    assert!(!manifest.required);
}

#[test]
fn a_worker_must_name_its_program() {
    let text = EMBED.replace(
        "[worker]\nprogram = \"miyu-embed\"\nargs = [\"serve\"]\n",
        "",
    );
    assert_eq!(wrong(&text), (Code::MissingKey, None));
    let text = EMBED.replace("program = \"miyu-embed\"\n", "");
    assert_eq!(wrong(&text), (Code::MissingKey, Some(6)));
    let text = EMBED.replace("\"miyu-embed\"", "\"bin/miyu-embed\"");
    assert_eq!(wrong(&text), (Code::BadProgram, Some(7)));
    let text = EMBED.replace(
        "args = [\"serve\"]",
        "args = [\"serve\"]\nstart = \"always\"",
    );
    assert_eq!(wrong(&text), (Code::UnknownKey, Some(9)));
}

#[test]
fn a_builtin_reads_what_it_recommends_and_depends_on() {
    let manifest = read(MEMORY).unwrap();
    assert_eq!(manifest.kind, PackageKind::Builtin);
    assert_eq!(manifest.recommends, ["embed"]);
    assert!(manifest.depends.is_empty());
    let text = MEMORY.replace("[recommends]", "[depends]");
    let manifest = read(&text).unwrap();
    assert_eq!(manifest.depends, ["embed"]);
    assert!(manifest.recommends.is_empty());
}

#[test]
fn dependencies_are_package_ids_listed_once() {
    let text = MEMORY.replace("[\"embed\"]", "[\"Embed\"]");
    assert_eq!(wrong(&text), (Code::BadDependency, Some(7)));
    let text = MEMORY.replace("[\"embed\"]", "[\"embed\", \"embed\"]");
    assert_eq!(wrong(&text), (Code::BadDependency, Some(7)));
    let text = MEMORY.replace("[\"embed\"]", "\"embed\"");
    assert_eq!(wrong(&text), (Code::NotTexts, Some(7)));
    let text = MEMORY.replace("workers = [\"embed\"]", "tools = [\"embed\"]");
    assert_eq!(wrong(&text), (Code::UnknownKey, Some(7)));
}

#[test]
fn a_process_package_reads_its_connection() {
    let manifest = read(BRIDGE).unwrap();
    assert_eq!(
        manifest.connection,
        Some(Connection {
            platform: "qq".to_string(),
        })
    );
    let text = BRIDGE.replace("platform = \"qq\"", "platform = \"QQ\"");
    assert_eq!(wrong(&text), (Code::BadPlatform, Some(15)));
    let text = BRIDGE.replace("platform = \"qq\"\n", "");
    assert_eq!(wrong(&text), (Code::MissingKey, Some(14)));
}

#[test]
fn only_builtins_are_required() {
    let text = BRIDGE.replace("kind = \"process\"", "kind = \"process\"\nrequired = true");
    assert_eq!(wrong(&text), (Code::WrongKind, Some(3)));
    let text = MEMORY.replace(
        "kind = \"builtin\"",
        "kind = \"builtin\"\nrequired = \"yes\"",
    );
    assert_eq!(wrong(&text), (Code::NotBool, Some(3)));
    let text = MEMORY.replace("kind = \"builtin\"", "kind = \"builtin\"\nrequired = false");
    assert!(!read(&text).unwrap().required);
}

#[test]
fn each_kind_writes_only_its_own_tables() {
    let command =
        "\n[command]\nname = \"memory\"\nprogram = \"miyu-memory\"\nabout = { en = \"M\" }\n";
    let check = format!("{command}\n[check]\nargs = [\"check\"]\n");
    let settings = "\n[settings.port]\ntype = \"int\"\nname = { en = \"Port\" }\n";
    let connection = "\n[connection]\nplatform = \"qq\"\n";
    let worker = "\n[worker]\nprogram = \"miyu-embed\"\n";
    for extra in [command, &check, settings, connection, worker] {
        assert_eq!(
            wrong(&format!("{MEMORY}{extra}")).0,
            Code::WrongKind,
            "内置包不能写：{extra}"
        );
    }
    for extra in [
        command,
        &check,
        settings,
        connection,
        "\n[recommends]\nworkers = [\"x\"]\n",
    ] {
        assert_eq!(
            wrong(&format!("{EMBED}{extra}")).0,
            Code::WrongKind,
            "小程序不能写：{extra}"
        );
    }
    let ui = "[package]\nkind = \"ui\"\nprotocol = [1, 1]\nname = { en = \"Web\" }\n";
    assert_eq!(wrong(&format!("{ui}{connection}")).0, Code::WrongKind);
    assert_eq!(wrong(&format!("{ui}{worker}")).0, Code::WrongKind);
    assert_eq!(
        wrong(&BRIDGE.replace("[connection]", "[worker]\nprogram = \"x\"\n\n[connection]")).0,
        Code::WrongKind
    );
}

#[test]
fn the_kind_is_one_of_four() {
    let text = MEMORY.replace("\"builtin\"", "\"daemon\"");
    let problem = read(&text).expect_err("不认识的种类");
    assert_eq!((problem.code, problem.line), (Code::BadKind, Some(2)));
    assert!(problem.message.contains("builtin"), "{}", problem.message);
}

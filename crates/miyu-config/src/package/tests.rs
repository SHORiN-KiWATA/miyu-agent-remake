//! 软件包清单的读法（施工 9-1 上）：每一格读成什么；每一种写错报哪个代码、第几行。

use super::*;

/// 终端界面的会话 2026-10-07 给的那份草稿。
const TUI: &str = r#"[package]
kind = "ui"
version = "0.0.1"
protocol = [1, 1]
name = { en = "Terminal interface", zh = "终端界面", ja = "ターミナル画面" }
summary = { en = "Chat with her in the terminal", zh = "在终端里和她对话" }

[command]
name = "tui"
program = "miyu-tui"
about = { en = "Open the terminal interface", zh = "打开终端界面", ja = "ターミナル画面を開く" }

[ui]
opens = ["config"]
"#;

/// 通讯平台的桥那种：核心拉起，带检查。
const BRIDGE: &str = r#"[package]
kind = "process"
protocol = [1, 2]
name = { en = "QQ bridge" }

[command]
name = "onebot"
program = "miyu-onebot"
about = { en = "QQ bridge over OneBot" }

[process]
args = ["serve"]
start = "manual"

[check]
args = ["check"]

[settings.token]
type = "secret"
layers = ["system"]
name = { en = "NapCat token" }
"#;

/// 读错了的：代码、第几行。
fn wrong(text: &str) -> (Code, Option<usize>) {
    let problem = read(text).expect_err("应该读不成");
    (problem.code, problem.line)
}

#[test]
fn a_ui_package_reads_every_field() {
    let manifest = read(TUI).unwrap();
    assert_eq!(manifest.kind, PackageKind::Ui);
    assert_eq!(manifest.version.as_deref(), Some("0.0.1"));
    assert_eq!(manifest.protocol, [1, 1]);
    assert_eq!(
        manifest.name.get("zh").map(String::as_str),
        Some("终端界面")
    );
    assert_eq!(manifest.summary.len(), 2);
    let command = manifest.command.unwrap();
    assert_eq!(
        (command.name.as_str(), command.program.as_str()),
        ("tui", "miyu-tui")
    );
    assert_eq!(command.about.len(), 3);
    let ui = manifest.ui.unwrap();
    assert_eq!(ui.opens, ["config"]);
    assert_eq!(ui.pages_dir, None);
    assert_eq!((manifest.process, manifest.check), (None, None));
}

#[test]
fn a_process_package_reads_its_start_check_and_settings() {
    let manifest = read(BRIDGE).unwrap();
    assert_eq!(manifest.kind, PackageKind::Process);
    assert_eq!(manifest.protocol, [1, 2]);
    assert_eq!(manifest.version, None);
    assert_eq!(
        manifest.process,
        Some(Process {
            args: vec!["serve".to_string()],
            start: Start::Manual,
        })
    );
    assert_eq!(
        manifest.check,
        Some(Check {
            args: vec!["check".to_string()],
        })
    );
    assert_eq!(manifest.settings.len(), 1);
    assert_eq!(manifest.settings[0].kind, SettingKind::Secret);
    let always = BRIDGE.replace("start = \"manual\"", "start = \"always\"");
    assert_eq!(read(&always).unwrap().process.unwrap().start, Start::Always);
    let bare = BRIDGE.replace("args = [\"serve\"]\nstart = \"manual\"\n", "");
    assert_eq!(
        read(&bare).unwrap().process,
        Some(Process {
            args: Vec::new(),
            start: Start::Manual,
        }),
        "都不写：没有参数、等开关"
    );
}

#[test]
fn the_smallest_manifest_has_a_kind_a_protocol_and_a_name() {
    let manifest =
        read("[package]\nkind = \"ui\"\nprotocol = [1, 1]\nname = { en = \"x\" }\n").unwrap();
    assert_eq!((manifest.command, manifest.ui), (None, None));
}

#[test]
fn broken_toml_and_unknown_or_misplaced_tables_say_where() {
    assert_eq!(wrong("[package\n"), (Code::Syntax, Some(1)));
    assert_eq!(
        wrong(&format!("{TUI}\n[voice]\nx = 1\n")).0,
        Code::UnknownTable
    );
    assert_eq!(wrong("package = 1\n"), (Code::NotATable, Some(1)));
    assert_eq!(wrong(&TUI.replace("[ui]", "[process]")).0, Code::WrongKind);
    assert_eq!(
        wrong(
            &BRIDGE
                .replace("[process]", "[ui]")
                .replace("start = \"manual\"\n", "")
        )
        .0,
        Code::WrongKind
    );
    assert_eq!(wrong("[command]\nname = \"x\"\n"), (Code::MissingKey, None));
}

#[test]
fn every_package_field_is_checked() {
    let missing_kind = TUI.replace("kind = \"ui\"\n", "");
    assert_eq!(wrong(&missing_kind), (Code::MissingKey, Some(1)));
    assert_eq!(
        wrong(&TUI.replace("\"ui\"", "\"daemon\"")),
        (Code::BadKind, Some(2))
    );
    assert_eq!(
        wrong(&TUI.replace("version = \"0.0.1\"", "version = 1")),
        (Code::NotText, Some(3))
    );
    for protocol in ["[2, 1]", "[1]", "[-1, 1]", "\"1\""] {
        let text = TUI.replace("[1, 1]", protocol);
        assert_eq!(wrong(&text), (Code::BadProtocol, Some(4)), "{protocol}");
    }
    assert_eq!(
        wrong(&TUI.replace("name = { en = \"Terminal", "name = { fr = \"Terminal")).0,
        Code::UnknownLanguage
    );
    assert_eq!(
        wrong(&TUI.replace("\nname = {", "\nnom = {")).0,
        Code::UnknownKey
    );
    let no_name = TUI.replace(
        "name = { en = \"Terminal interface\", zh = \"终端界面\", ja = \"ターミナル画面\" }\n",
        "",
    );
    assert_eq!(wrong(&no_name), (Code::MissingKey, Some(1)));
    assert_eq!(
        wrong(&TUI.replace("summary = {", "summary = 1 #")).0,
        Code::NotPhrases
    );
}

#[test]
fn every_command_field_is_checked() {
    for name in ["\"Tui\"", "\"1tui\"", "\"t_ui\"", "\"\"", "1"] {
        let text = TUI.replace("name = \"tui\"", &format!("name = {name}"));
        assert_eq!(wrong(&text).0, Code::BadCommandName, "{name}");
    }
    for program in ["\"bin/miyu-tui\"", "\"bin\\\\miyu-tui\"", "\"\"", "\"..\""] {
        let text = TUI.replace("\"miyu-tui\"", program);
        assert_eq!(wrong(&text).0, Code::BadProgram, "{program}");
    }
    let no_about = TUI.replace(
        "about = { en = \"Open the terminal interface\", zh = \"打开终端界面\", ja = \"ターミナル画面を開く\" }\n",
        "",
    );
    assert_eq!(wrong(&no_about), (Code::MissingKey, Some(8)));
}

#[test]
fn process_ui_and_check_fields_are_checked() {
    assert_eq!(
        wrong(&BRIDGE.replace("\"manual\"", "\"later\"")).0,
        Code::BadStart
    );
    assert_eq!(
        wrong(&BRIDGE.replace("[\"serve\"]", "\"serve\"")).0,
        Code::NotTexts
    );
    assert_eq!(
        wrong(&BRIDGE.replace("args = [\"check\"]", "args = [1]")).0,
        Code::NotTexts
    );
    let no_command = "[package]\nkind = \"process\"\nprotocol = [1, 1]\nname = { en = \"x\" }\n\n[process]\nargs = []\n";
    assert_eq!(wrong(no_command), (Code::NeedsCommand, Some(6)));
    let check_alone = "[package]\nkind = \"ui\"\nprotocol = [1, 1]\nname = { en = \"x\" }\n\n[check]\nargs = []\n";
    assert_eq!(wrong(check_alone), (Code::NeedsCommand, Some(6)));
    for page in ["\"Config\"", "\"\"", "1"] {
        let text = TUI.replace("\"config\"", page);
        assert_eq!(wrong(&text).0, Code::BadPage, "{page}");
    }
    for dir in [
        "\"/srv/web\"",
        "\"../web\"",
        "\"web/../../x\"",
        "\"\"",
        "\"C:\\\\web\"",
    ] {
        let text = format!("{TUI}pages_dir = {dir}\n");
        assert_eq!(wrong(&text).0, Code::BadPagesDir, "{dir}");
    }
    let pages = format!("{TUI}pages_dir = \"web/pages\"\n");
    assert_eq!(
        read(&pages).unwrap().ui.unwrap().pages_dir.as_deref(),
        Some("web/pages")
    );
    assert_eq!(
        wrong(&format!("{TUI}theme = \"dark\"\n")).0,
        Code::UnknownKey
    );
}

#[test]
fn every_code_has_a_name_and_the_message_is_english() {
    let problem = read(&TUI.replace("\"ui\"", "\"daemon\"")).unwrap_err();
    assert_eq!(problem.code.as_str(), "bad_kind");
    assert_eq!(problem.detail, "daemon");
    assert_eq!(
        problem.message,
        "package.kind must be ui or process, not \"daemon\""
    );
    assert_eq!(
        problem.to_string(),
        "line 2: package.kind must be ui or process, not \"daemon\""
    );
}

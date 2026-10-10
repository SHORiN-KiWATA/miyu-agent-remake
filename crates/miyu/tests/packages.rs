//! 软件包加的子命令（施工 9-2，`docs/blueprint/packages.md`「转交」）：真二进制走一遍。管理员家目录里放一份清单，子命令
//! `probe` 跑的程序就是 `miyu` 自己（它在 `miyu` 旁边）：参数原样交过去、退出码照它的；`miyu help probe` 转成 `--help`；
//! `miyu -h` 多一节，撞了内置的不列；没装的程序说没装、退出码 1；不认识的照旧退出码 2，不拉起核心。

use std::process::{Command, Output};

use crate::support::{Home, MIYU};

/// 一份清单：子命令 `name` 跑 `program`。
fn manifest(name: &str, program: &str, about: &str) -> String {
    format!(
        "[package]\nkind = \"ui\"\nprotocol = [1, 1]\nname = {{ en = \"P\" }}\n\n[command]\nname = \"{name}\"\nprogram = \"{program}\"\nabout = {{ en = \"{about}\", zh = \"{about}（中）\" }}\n"
    )
}

fn install(home: &Home, id: &str, text: &str) {
    let dir = home.root.path().join("home/admin/packages");
    std::fs::create_dir_all(&dir).expect("建得了目录");
    std::fs::write(dir.join(format!("{id}.toml")), text).expect("写得进");
}

/// 在临时的数据根上跑 `miyu <args>`，界面语言是 `lang`。
fn miyu(home: &Home, lang: &str, args: &[&str]) -> Output {
    Command::new(MIYU)
        .args(args)
        .env("MIYU_HOME", home.root.path())
        .env("MIYU_RESOURCES", crate::support::resources())
        .envs(crate::support::offline(home.root.path()))
        .env("LANG", lang)
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .output()
        .expect("跑得起来")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn a_package_command_runs_its_program_with_the_same_arguments() {
    let home = Home::new();
    install(&home, "probe", &manifest("probe", "miyu", "Probe it"));
    let version = miyu(&home, "C", &["probe", "--version"]);
    assert_eq!(version.status.code(), Some(0), "{}", text(&version.stderr));
    assert!(
        text(&version.stdout).starts_with("miyu "),
        "参数原样交过去：{}",
        text(&version.stdout)
    );
    let wrong = miyu(&home, "C", &["probe", "nonsense"]);
    assert_eq!(wrong.status.code(), Some(2), "退出码照它的");
    assert!(
        text(&wrong.stderr).contains("nonsense"),
        "{}",
        text(&wrong.stderr)
    );
    let help = miyu(&home, "C", &["help", "probe"]);
    assert_eq!(help.status.code(), Some(0));
    assert!(
        text(&help.stdout).starts_with("Usage: miyu"),
        "帮助由它自己说：{}",
        text(&help.stdout)
    );
    assert!(!home.root.run().join("socket").exists(), "没拉起核心");
    assert!(home.core_log().is_empty(), "核心没起来过");
}

#[test]
fn the_help_page_lists_package_commands_but_not_ones_taken_by_builtins() {
    let home = Home::new();
    install(&home, "probe", &manifest("probe", "miyu", "Probe it"));
    install(&home, "taken", &manifest("ask", "miyu", "Not me"));
    let zh = text(&miyu(&home, "zh_CN.UTF-8", &["-h"]).stdout);
    assert!(
        zh.contains("\n\n软件包加的命令：\n  onebot                开、关、查看接入QQ\n  probe                 Probe it（中）\n  tui                   打开终端界面\n\n"),
        "{zh}"
    );
    let en = text(&miyu(&home, "C", &["--help"]).stdout);
    assert!(
        en.contains("\n\nCommands from packages:\n  onebot                Start, stop and look at Connect QQ\n  probe                 Probe it\n  tui                   Open the terminal interface\n\n"),
        "{en}"
    );
    assert!(!en.contains("Not me"), "撞了内置的不列：{en}");
    assert!(
        !en.contains("Commands from packages:\n  web"),
        "出厂网页那一份撞了内置的 web，不列"
    );
    // 内置的优先：`miyu ask` 照旧是内置的那个。
    let ask = miyu(&home, "C", &["ask", "--help"]);
    assert!(
        text(&ask.stdout).starts_with("Usage: miyu ask"),
        "{}",
        text(&ask.stdout)
    );
    // 只有出厂的：终端的清单带着子命令 tui（9-3 补）、桥的带着 onebot（施工 O-18），这一节只有这两个。一个都没有的不写这一节，
    // 由 miyu-cli 的单元测试守着。
    let plain = Home::new();
    let shipped = text(&miyu(&plain, "C", &["-h"]).stdout);
    assert!(
        shipped.contains(
            "\n\nCommands from packages:\n  onebot                Start, stop and look at Connect QQ\n  tui                   Open the terminal interface\n\n"
        ),
        "{shipped}"
    );
}

#[test]
fn a_missing_program_is_said_and_an_unknown_word_is_still_refused() {
    let home = Home::new();
    install(
        &home,
        "ghost",
        &manifest("ghost", "miyu-no-such-program-anywhere", "Ghost"),
    );
    let missing = miyu(&home, "C", &["ghost"]);
    assert_eq!(missing.status.code(), Some(1));
    let said = text(&missing.stderr);
    assert!(
        said.starts_with("miyu-no-such-program-anywhere not found: ")
            && said.contains("ghost.toml says miyu ghost runs it"),
        "{said}"
    );
    let unknown = miyu(&home, "C", &["hello"]);
    assert_eq!(unknown.status.code(), Some(2), "不认识的照旧拒");
    assert!(!home.root.run().join("socket").exists(), "没拉起核心");
}

/// 清单是内置包、这一份核心没编进它的代码（施工 F-2，设计 30 第二节第 3 条）：真核心起来时照读坏了的清单记一行
/// `WARN package invalid`，带 `not_built_in`；编进来了的出厂内置包不记。
#[tokio::test]
async fn the_core_says_which_builtin_it_lacks() {
    let home = Home::new();
    install(
        &home,
        "xghost",
        "[package]\nkind = \"builtin\"\nprotocol = [1, 1]\nname = { en = \"Ghost\" }\n",
    );
    let (connection, token) = crate::support::within(
        "拉起",
        miyu_ipc::connect_or_start(&home.root, || home.core()),
    )
    .await
    .expect("拉得起");
    let reply = crate::support::hello(connection, &token).await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_stopped().await;
    let log = home.core_log();
    let lacking: Vec<&str> = log
        .lines()
        .filter(|line| line.contains("package invalid"))
        .collect();
    assert_eq!(lacking.len(), 1, "{log}");
    assert!(lacking[0].contains("xghost"), "{log}");
    assert!(lacking[0].contains("built-in package xghost"), "{log}");
    assert_eq!(
        crate::support::count(&log, "required package missing"),
        0,
        "{log}"
    );
}

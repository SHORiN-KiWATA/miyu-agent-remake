//! 三个入口照清单找（施工 9-3，`docs/blueprint/cli/main.md`「怎么走」）：真二进制在伪终端里走一遍。不带子命令的 `miyu`
//! 照 `ui.head` 拉起清单里的界面、不带参数；`miyu config` 带 `--page config`，清单没认这一页的照旧印帮助；`ui.head` 指着
//! 没装的说装了哪几个、退出码 1。界面是放在 `miyu` 旁边的一个临时脚本（程序只找 `miyu` 旁边的），它记下参数、以 7 退出。
//! 只在 Unix 上跑：要伪终端。

#![cfg(unix)]

mod support;

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};

use support::{Home, MIYU, offline, resources};

/// 一对伪终端：主端，从端的路径。
fn pty() -> (File, String) {
    let main = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("开得起伪终端");
    grantpt(&main).expect("放得开");
    unlockpt(&main).expect("解得开");
    let name = ptsname(&main, Vec::new()).expect("问得到名字");
    (
        File::from(main),
        name.to_str().expect("是 UTF-8").to_string(),
    )
}

/// 主端一直读着、读到的扔掉：没人读的伪终端写满缓冲，写的一方就卡住（macOS 的缓冲比帮助页小，施工 9-3 在 CI 上卡过）。
/// 从端都关了读就报错或读到头，线程跟着结束。
fn drain(main: File) {
    std::thread::spawn(move || {
        let mut sink = [0u8; 4096];
        while matches!((&main).read(&mut sink), Ok(read) if read > 0) {}
    });
}

fn secondary(path: &str) -> Stdio {
    Stdio::from(
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("开得起从端"),
    )
}

/// `miyu` 旁边的一个临时界面：把参数写进 `seen`、以 7 退出。用完删掉。
struct Head {
    program: PathBuf,
    seen: PathBuf,
}

impl Head {
    fn new(home: &Home) -> Head {
        let dir = std::fs::canonicalize(MIYU)
            .expect("主程序在")
            .parent()
            .expect("有上一级")
            .to_path_buf();
        // 同一个进程里几条测试一起跑：名字带上第几个，各用各的。
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let program = dir.join(format!("miyu-test-head-{}-{n}", std::process::id()));
        let seen = home.dir.join("seen");
        std::fs::write(
            &program,
            format!("#!/bin/sh\necho \"$@\" > '{}'\nexit 7\n", seen.display()),
        )
        .expect("写得进");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("改得了");
        Head { program, seen }
    }

    fn name(&self) -> String {
        self.program
            .file_name()
            .expect("有名字")
            .to_string_lossy()
            .into_owned()
    }

    /// 它上一次收到的参数；没跑过的没有。
    fn seen(&self) -> Option<String> {
        let text = std::fs::read_to_string(&self.seen).ok()?;
        drop(std::fs::remove_file(&self.seen));
        Some(text.trim_end().to_string())
    }
}

impl Drop for Head {
    fn drop(&mut self) {
        drop(std::fs::remove_file(&self.program));
    }
}

/// 管理员家目录里的界面清单：编号 `id`，程序 `program`，认 `opens` 那几页。
fn install(home: &Home, id: &str, program: &str, opens: &str) {
    let dir = home.root.path().join("home/admin/packages");
    std::fs::create_dir_all(&dir).expect("建得了目录");
    std::fs::write(
        dir.join(format!("{id}.toml")),
        format!(
            "[package]\nkind = \"ui\"\nprotocol = [1, 1]\nname = {{ en = \"T\" }}\n\n[command]\nname = \"{id}\"\nprogram = \"{program}\"\nabout = {{ en = \"T\" }}\n\n[ui]\nopens = [{opens}]\n"
        ),
    )
    .expect("写得进");
}

/// 测试的界面用的编号：避开出厂会有的（出厂带着 `tui`，同编号认出厂的，9-3 补）。系统配置里 `ui.head` 指着它。
fn pointed(home: &Home) {
    home.system_config("[ui]\nhead = \"probe-head\"\n");
}

/// 在伪终端里跑 `miyu <args>`：标准输入、输出接伪终端，标准错误接管道（好读）。
fn miyu(home: &Home, args: &[&str]) -> Output {
    miyu_with(home, args, true)
}

/// 同 [`miyu`]；`stdin` 不是的，标准输入接空的（只有标准输出是终端）。
fn miyu_with(home: &Home, args: &[&str], stdin: bool) -> Output {
    let (main, path) = pty();
    drain(main);
    let input = if stdin {
        secondary(&path)
    } else {
        Stdio::null()
    };
    Command::new(MIYU)
        .args(args)
        .env("MIYU_HOME", home.root.path())
        .env("MIYU_RESOURCES", resources())
        .envs(offline(home.root.path()))
        .env("LANG", "zh_CN.UTF-8")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .stdin(input)
        .stdout(secondary(&path))
        .stderr(Stdio::piped())
        .output()
        .expect("跑得起来")
}

fn said(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[tokio::test]
async fn plain_miyu_and_miyu_config_open_the_head_from_its_manifest() {
    let home = Home::new();
    let head = Head::new(&home);
    pointed(&home);
    install(&home, "probe-head", &head.name(), "\"config\"");
    let plain = miyu(&home, &[]);
    assert_eq!(
        plain.status.code(),
        Some(7),
        "照它的退出码：{}",
        said(&plain)
    );
    assert_eq!(head.seen().as_deref(), Some(""), "不带参数");
    let config = miyu(&home, &["config"]);
    assert_eq!(config.status.code(), Some(7), "{}", said(&config));
    assert_eq!(head.seen().as_deref(), Some("--page config"));
    home.kill_core().await;
}

#[tokio::test]
async fn a_head_without_the_config_page_leaves_miyu_config_to_the_help() {
    let home = Home::new();
    let head = Head::new(&home);
    pointed(&home);
    install(&home, "probe-head", &head.name(), "");
    let config = miyu(&home, &["config"]);
    assert_eq!(config.status.code(), Some(2), "{}", said(&config));
    assert_eq!(head.seen(), None, "没拉起");
    home.kill_core().await;
}

#[tokio::test]
async fn a_missing_head_names_the_installed_ones() {
    let home = Home::new();
    let head = Head::new(&home);
    install(&home, "other", &head.name(), "");
    home.system_config("[ui]\nhead = \"nope\"\n");
    let plain = miyu(&home, &[]);
    assert_eq!(plain.status.code(), Some(1));
    // 出厂的终端只有清单、程序随 M9，不算装了；网页的程序编没编出来看这一次构建（9-3 补）。
    let installed = match beside("miyu-web") {
        true => "other、web",
        false => "other",
    };
    assert_eq!(
        said(&plain).trim_end(),
        format!(
            "没装 nope 这个界面（ui.head 指着它）。装上它的软件包，或者 miyu config set ui.head <编号> 换成装了的界面：{installed}。"
        )
    );
    assert_eq!(head.seen(), None);
    home.kill_core().await;
}

#[tokio::test]
async fn the_shipped_terminal_without_its_program_says_where_it_should_be() {
    // 出厂的 ui.head 是 tui，出厂带着它的清单，程序 miyu-tui 随 M9（9-3 补）。
    let home = Home::new();
    assert!(!beside("miyu-tui"), "这一次构建里没有终端的程序");
    let plain = miyu(&home, &[]);
    assert_eq!(plain.status.code(), Some(1), "{}", said(&plain));
    assert!(
        said(&plain).starts_with(
            "tui 这个界面的程序 miyu-tui 不在 miyu 旁边（ui.head 指着它）。把它放到 miyu 旁边，或者 miyu config set ui.head <编号> 换成装了的界面"
        ),
        "{}",
        said(&plain)
    );
    home.kill_core().await;
}

/// `miyu` 真实位置旁边有没有 `program`。
fn beside(program: &str) -> bool {
    std::fs::canonicalize(MIYU)
        .expect("主程序在")
        .with_file_name(format!("{program}{}", std::env::consts::EXE_SUFFIX))
        .is_file()
}

#[tokio::test]
async fn only_a_terminal_on_both_ends_opens_the_head() {
    let home = Home::new();
    let head = Head::new(&home);
    pointed(&home);
    install(&home, "probe-head", &head.name(), "\"config\"");
    let piped = miyu_with(&home, &[], false);
    assert_eq!(piped.status.code(), Some(2), "标准输入不是终端：印帮助");
    assert_eq!(head.seen(), None, "没拉起");
    let config = miyu_with(&home, &["config"], false);
    assert_eq!(config.status.code(), Some(2));
    assert_eq!(head.seen(), None);
}

//! 真核心照开关拉起真的桥（施工 O-18，`onebot.md` 第一条「对外的样子」「怎么走」第 1、11 条，`extensions.md`）：测试程序里的
//! 核心照出厂的清单拉起硬链接在测试程序旁边的 `miyu-onebot`，`start`、`stop`、`restart`、`status` 是真的程序。`start` 以后
//! NapCat 连得进来、主人的私聊照旧来回，`status` 说在跑、NapCat 连着；`stop` 以后桥自己退出、端口关了；桥被杀掉，核心拉起新的
//! 一个，NapCat 重连得上；端口被占，核心停下，`status` 说是配置错、带出「端口被占」那一句；关着的不能 `restart`。不靠墙钟睡，
//! 等状态。

use std::time::Duration;

use miyu_onebot::control::{Halt, Report};
use miyu_onebot::texts::Texts;
use miyu_session::testkit::{Play, Script};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::support::spawning::*;
use crate::support::*;

/// 照中文说的那一句。
fn zh(report: &Report) -> String {
    Texts::load(ResourceRoot::at(resources()), "zh")
        .expect("读得出来")
        .report(report)
}

/// 一个照开关拉起扩展的核心，两个端口照 `listen`、`web`，令牌是 [`TOKEN`]，说中文。
fn home(script: &Script, listen: u16, web: u16) -> Home {
    let home = Home::spawning(script, &ports_config(listen, web));
    store_token(&home.root, TOKEN);
    home
}

/// 跑 `miyu-onebot <args>`：退出码是 0，交回标准输出。
async fn ok(root: &DataRoot, args: &[&str]) -> String {
    let ran = cli(root, args).await;
    assert_eq!(
        ran.status.code(),
        Some(0),
        "{args:?}：{}{}",
        text(&ran.stdout),
        text(&ran.stderr)
    );
    text(&ran.stdout)
}

/// 等桥在跑、NapCat 的端口开着，交回它的进程号。
async fn until_running(root: &DataRoot, listen: u16) -> u64 {
    let running = until_extension(root, |one| one["state"] == "running").await;
    until_port(listen, true).await;
    running["pid"].as_u64().expect("在跑的有进程号")
}

#[tokio::test]
async fn start_lets_the_core_run_the_bridge_and_stop_closes_it() {
    let (listen, web) = (free_port(), free_port());
    let home = home(&Script::new([Play::Says("在。")]), listen, web);
    assert_eq!(
        ok(&home.root, &["status"]).await,
        format!("{}\n", zh(&Report::Off))
    );
    let started = ok(&home.root, &["start"]).await;
    assert!(
        started.starts_with(&format!("{}\n", zh(&Report::Started))),
        "{started}"
    );
    let pid = until_running(&home.root, listen).await;
    let mut napcat = owner_napcat(listen).await;
    napcat.owner_says(1, "在吗").await;
    assert_eq!(napcat.reply().await, "在。", "主人的私聊照旧来回");
    let status = until_status(&home.root, |out| out.contains("NapCat.Onebot")).await;
    let expected = [
        zh(&Report::Running(pid)),
        zh(&Report::Napcat {
            implementation: "NapCat.Onebot".to_string(),
            version: "4.8.0".to_string(),
            bot: BOT.to_string(),
        }),
        zh(&Report::Ports {
            listen: u64::from(listen),
            web: u64::from(web),
        }),
    ];
    assert_eq!(status, format!("{}\n", expected.join("\n")));
    assert_eq!(
        ok(&home.root, &["stop"]).await,
        format!("{}\n", zh(&Report::Stopped))
    );
    // `stop` 等桥退出才回：这时端口已经关了。
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", listen))
            .await
            .is_err(),
        "桥退出了，端口关了"
    );
    assert_eq!(
        ok(&home.root, &["status"]).await,
        format!("{}\n", zh(&Report::Off))
    );
    let log = std::fs::read_to_string(home.root.state().join("logs").join("onebot.log"))
        .expect("桥写了运行日志");
    assert!(
        log.contains("core closed, stopping"),
        "标准输入读到头，桥自己停下，不是被杀的：{log}"
    );
    napcat.close().await;
    home.stop_extensions().await;
}

#[tokio::test]
async fn a_killed_bridge_is_started_again_and_napcat_comes_back() {
    let (listen, web) = (free_port(), free_port());
    let home = home(&Script::new([Play::Says("回来了。")]), listen, web);
    ok(&home.root, &["start"]).await;
    let first = until_running(&home.root, listen).await;
    kill(first);
    let again = until_extension(&home.root, |one| {
        one["state"] == "running" && one["pid"].as_u64() != Some(first)
    })
    .await;
    assert_eq!(again["failures"], 1, "被杀掉算一次失败：{again}");
    until_port(listen, true).await;
    let mut napcat = owner_napcat(listen).await;
    napcat.owner_says(2, "还在吗").await;
    assert_eq!(napcat.reply().await, "回来了。", "NapCat 重连得上");
    napcat.close().await;
    ok(&home.root, &["stop"]).await;
    home.stop_extensions().await;
}

#[tokio::test]
async fn a_port_in_use_stops_it_and_status_says_why() {
    let (listen, web) = (free_port(), free_port());
    let taken = std::net::TcpListener::bind(("127.0.0.1", listen)).expect("占得上");
    let home = home(&Script::new([]), listen, web);
    ok(&home.root, &["start"]).await;
    let stopped = until_extension(&home.root, |one| one["state"] == "stopped").await;
    assert_eq!(stopped["reason"], "config_error", "{stopped}");
    let status = ok(&home.root, &["status"]).await;
    let mut lines = status.lines();
    assert_eq!(
        lines.next(),
        Some(zh(&Report::Halted(Halt::ConfigError)).as_str())
    );
    assert_eq!(lines.next(), Some(zh(&Report::Stderr).as_str()));
    let port_in_use = Texts::load(ResourceRoot::at(resources()), "zh")
        .expect("读得出来")
        .failure(&miyu_onebot::serve::Failure::PortInUse(listen));
    assert!(
        status.contains(&format!("  {port_in_use}\n")),
        "带出标准错误里「端口被占」那一句：{status}"
    );
    drop(taken);
    home.stop_extensions().await;
}

#[tokio::test]
async fn restarting_an_extension_that_is_off_is_refused_in_the_cores_words() {
    let (listen, web) = (free_port(), free_port());
    let home = home(&Script::new([]), listen, web);
    let refused = cli(&home.root, &["restart"]).await;
    assert_eq!(refused.status.code(), Some(1));
    assert_eq!(text(&refused.stdout), "");
    assert_eq!(
        text(&refused.stderr),
        "这个扩展关着，先打开它。\n",
        "照核心的原话（握手说中文）"
    );
    // 开了再重启：换一个新进程。
    ok(&home.root, &["start"]).await;
    let first = until_running(&home.root, listen).await;
    let restarted = ok(&home.root, &["restart"]).await;
    assert!(
        restarted.starts_with(&format!("{}\n", zh(&Report::Restarted))),
        "{restarted}"
    );
    until_extension(&home.root, |one| {
        one["state"] == "running" && one["pid"].as_u64() != Some(first)
    })
    .await;
    ok(&home.root, &["stop"]).await;
    tokio::time::timeout(Duration::from_secs(10), home.stop_extensions())
        .await
        .expect("停得下");
}

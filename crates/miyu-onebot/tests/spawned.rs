//! 真核心照开关拉起真的桥（施工 O-18，`onebot.md` 第一条「对外的样子」「怎么走」第 1、11 条，`extensions.md`）：测试程序里的
//! 核心照出厂的清单拉起硬链接在测试程序旁边的 `miyu-onebot`，`start`、`stop`、`restart`、`status` 是真的程序。`start` 以后
//! NapCat 连得进来、主人的私聊照旧来回，`status` 说在跑、NapCat 连着；`stop` 以后桥自己退出、端口关了；桥被杀掉，核心拉起新的
//! 一个，NapCat 重连得上；端口被占，核心停下，`status` 说是配置错、带出「端口被占」那一句；关着的不能 `restart`。核心改了桥的
//! 配置（施工 O-20）：令牌、两个端口不重启当场换，令牌删了一律 401。不靠墙钟睡，等状态。挑的空端口在桥起来以前被别人占了的，
//! 换一组从头再来（`support/ports.rs`）。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_onebot::control::{Halt, Report};
use miyu_onebot::status_file;
use miyu_onebot::texts::Texts;
use miyu_session::testkit::{Play, Script};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::support::http::get;
use crate::support::ports::{TRIES, on_free_ports};
use crate::support::spawning::*;
use crate::support::*;

/// 只换了值的令牌（`secret.set`）。
const NEW: &str = "napcat-new-token";

/// 换了引用的令牌（`config.set` 引用别的密钥）。
const OTHER: &str = "napcat-other-token";

/// 正向的等待最多多久：真的程序、真的核心，负载高时慢。
const WAIT: Duration = Duration::from_secs(60);

/// 照中文说的那一句。
fn zh(report: &Report) -> String {
    Texts::load(ResourceRoot::at(resources()), "zh")
        .expect("读得出来")
        .report(report)
}

/// 一个照开关拉起扩展的核心，两个端口照 `listen`、`web`，令牌是 [`TOKEN`]，说中文。
fn home(script: &Script, listen: u16, web: u16) -> Home {
    Home::spawning(script, &ports_config(listen, web))
}

/// 经核心调一次 `method`（`secret.set`、`config.set`），照页面、命令行的样子：核心收了才回。
async fn core_call(root: &DataRoot, method: &str, params: Value) {
    let mut core = miyu_webserve::open::Core::connect_running(root, "test")
        .await
        .expect("连得上核心");
    core.call("o20", method, params)
        .await
        .unwrap_or_else(|refused| panic!("{method} 被拒：{refused}"));
}

/// 拿 `token` 连 `port`（不报号：不顶掉连着的那一条），直到进得去。
async fn until_admitted(port: u16, token: &str) -> NapCat {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        match napcat(port, "/ws", Auth::Bearer(token), None).await {
            Ok(napcat) => return napcat,
            Err(401) => {}
            Err(status) => panic!("回的不是 401：{status}"),
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{port} 上一直不认 {token}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// 拿 `token` 连 `port`，直到被拒（401）。
async fn until_refused(port: u16, token: &str) {
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        match napcat(port, "/ws", Auth::Bearer(token), None).await {
            Err(401) => return,
            Ok(napcat) => napcat.close().await,
            Err(status) => panic!("回的不是 401：{status}"),
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "{port} 上一直收 {token}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// 经核心把 `key`（`onebot.listen`、`onebot.web`）改成挑的一个空端口，等桥当场换上：状态文件里的 `field` 是它。桥说它被占了
/// （运行日志 `apply port in use port=<它>`：挑来放掉以后被别人先拿走了，`support/ports.rs`）的换一个再来，最多 [`TRIES`] 次。
/// 交回换上的端口。
async fn moved(root: &DataRoot, key: &str, field: &str) -> u16 {
    let log = root.state().join("logs").join("onebot.log");
    for _ in 0..TRIES {
        let new = free_port();
        let change = json!({"layer": "system", "changes": [{"key": key, "value": new}]});
        core_call(root, "config.set", change).await;
        let in_use = format!("apply port in use port={new}");
        let deadline = tokio::time::Instant::now() + WAIT;
        loop {
            if status_file::read(root).is_some_and(|file| file[field] == new) {
                return new;
            }
            if std::fs::read_to_string(&log).is_ok_and(|log| log.contains(&in_use)) {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "桥一直没换 {key}：{:?}",
                status_file::read(root)
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    panic!("试了 {TRIES} 个端口，桥都说被占了");
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

#[tokio::test]
async fn start_lets_the_core_run_the_bridge_and_stop_closes_it() {
    let script = Script::new([Play::Says("在。")]);
    let (home, listen, web, pid) = on_free_ports(async |listen, web| {
        let home = home(&script, listen, web);
        assert_eq!(
            ok(&home.root, &["status"]).await,
            format!("{}\n", zh(&Report::Off))
        );
        let started = ok(&home.root, &["start"]).await;
        assert!(
            started.starts_with(&format!("{}\n", zh(&Report::Started))),
            "{started}"
        );
        let pid = bridge_up(&home.root, listen, web, None).await?;
        Ok((home, listen, web, pid))
    })
    .await;
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
    let script = Script::new([Play::Says("回来了。")]);
    // 核心拉起的新桥照样绑这两个端口：杀掉以后到它绑上之间被别人占了的，也换一组从头再来。
    let (home, listen) = on_free_ports(async |listen, web| {
        let home = home(&script, listen, web);
        ok(&home.root, &["start"]).await;
        let first = bridge_up(&home.root, listen, web, None).await?;
        kill(first);
        bridge_up(&home.root, listen, web, Some(first)).await?;
        Ok((home, listen))
    })
    .await;
    let again = extension(&home.root).await;
    assert_eq!(again["failures"], 1, "被杀掉算一次失败：{again}");
    let mut napcat = owner_napcat(listen).await;
    napcat.owner_says(2, "还在吗").await;
    assert_eq!(napcat.reply().await, "回来了。", "NapCat 重连得上");
    napcat.close().await;
    ok(&home.root, &["stop"]).await;
    home.stop_extensions().await;
}

#[tokio::test]
async fn a_port_in_use_stops_it_and_status_says_why() {
    // 占着的端口由测试自己从系统挑来、一直拿着，没有放掉再绑的空当；桥先绑 NapCat 的这一个，绑不上就停，WebUI 的那个用不上。
    let taken = std::net::TcpListener::bind("127.0.0.1:0").expect("挑得到");
    let listen = taken.local_addr().expect("有地址").port();
    let web = free_port();
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
    let script = Script::new([]);
    let home = on_free_ports(async |listen, web| {
        let home = home(&script, listen, web);
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
        let first = bridge_up(&home.root, listen, web, None).await?;
        let restarted = ok(&home.root, &["restart"]).await;
        assert!(
            restarted.starts_with(&format!("{}\n", zh(&Report::Restarted))),
            "{restarted}"
        );
        bridge_up(&home.root, listen, web, Some(first)).await?;
        Ok(home)
    })
    .await;
    ok(&home.root, &["stop"]).await;
    tokio::time::timeout(Duration::from_secs(10), home.stop_extensions())
        .await
        .expect("停得下");
}

#[tokio::test]
async fn the_running_bridge_takes_changes_from_the_core_without_a_restart() {
    let script = Script::new([Play::Says("在。"), Play::Says("还在。")]);
    let (home, listen, web) = on_free_ports(async |listen, web| {
        let home = home(&script, listen, web);
        ok(&home.root, &["start"]).await;
        bridge_up(&home.root, listen, web, None).await?;
        Ok((home, listen, web))
    })
    .await;
    // 照握手交的：配置里写的两个端口、密钥文件里的令牌。
    let mut open = owner_napcat(listen).await;
    // 只换密钥的值（`secret.set`，引用不变）：新的收、旧的拒，连着的那一条照样收发。
    core_call(
        &home.root,
        "secret.set",
        json!({"name": "onebot", "value": NEW}),
    )
    .await;
    until_admitted(listen, NEW).await.close().await;
    let old = napcat(listen, "/ws", Auth::Bearer(TOKEN), None).await;
    assert_eq!(old.err(), Some(401), "旧的不收");
    open.owner_says(1, "在吗").await;
    assert_eq!(open.reply().await, "在。", "已经连着的那一条照样收发");
    // 引用换成别的密钥（`config.set`）。
    core_call(
        &home.root,
        "secret.set",
        json!({"name": "other", "value": OTHER}),
    )
    .await;
    let other = json!({"layer": "system", "changes": [{"key": "onebot.token", "value": {"secret": "other"}}]});
    core_call(&home.root, "config.set", other).await;
    until_admitted(listen, OTHER).await.close().await;
    let old = napcat(listen, "/ws", Auth::Bearer(NEW), None).await;
    assert_eq!(old.err(), Some(401), "换下来的不收");
    // NapCat 的端口：当场换，旧的关了，连着的那一条照样收发。
    let new_listen = moved(&home.root, "onebot.listen", "listen").await;
    until_port(listen, false).await;
    until_admitted(new_listen, OTHER).await.close().await;
    open.owner_says(2, "还在吗").await;
    assert_eq!(open.reply().await, "还在。", "换端口不断连着的");
    // WebUI 的端口：新地址上有页面，旧的关了。
    let new_web = moved(&home.root, "onebot.web", "web").await;
    until_port(web, false).await;
    assert_eq!(get(new_web, "/", &[]).await.status, 200, "新地址上有页面");
    let ports = zh(&Report::Ports {
        listen: u64::from(new_listen),
        web: u64::from(new_web),
    });
    until_status(&home.root, |out| out.contains(&ports)).await;
    // 删了令牌：以后连进来的一律 401。
    let unset = json!({"layer": "system", "changes": [{"key": "onebot.token", "unset": true}]});
    core_call(&home.root, "config.set", unset).await;
    until_refused(new_listen, OTHER).await;
    open.close().await;
    ok(&home.root, &["stop"]).await;
    home.stop_extensions().await;
}

//! 令牌没设（施工 O-16 补、补二，`onebot.md` 第一条「怎么走」第 1、2 条、「施工时定的」第 3 条，第二条「施工时定的」第 14、
//! 18 条；`18-通讯平台.md` 第三节「还没配好就 `start`：桥只开 WebUI，不连 NapCat，等配好」）：桥照样起来，两个端口都开，说一句
//! 怎么办；NapCat 连进来 401。照页面的办法写进令牌，不重启，NapCat 下一次连就通；再换一个，新的进得来、旧的不收，已经连着的
//! 那一条照样收发。真的程序 `miyu-onebot serve` 照常起来（标准错误、运行日志各一句），改了配置文件，`/status` 的 `token`
//! 跟着变、NapCat 不用重启就连上，`/token` 交出值、运行日志里没有它；`miyu-onebot web --print` 照常印网址。

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::json;
use tokio::net::TcpStream;

use miyu_onebot::serve::{Notice, Serve};
use miyu_onebot::texts::Texts;
use miyu_session::testkit::{Play, Script};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::support::fake_core::{CODE, LOGIN, fake_core};
use crate::support::http::*;
use crate::support::*;

/// 换上的新令牌。
const NEW: &str = "napcat-new-token";

/// 一个空着的端口：系统挑一个，马上放掉。
fn free_port() -> u16 {
    let free = std::net::TcpListener::bind("127.0.0.1:0").expect("挑得到");
    free.local_addr().expect("有地址").port()
}

/// 带着登录令牌 [`LOGIN`] 取一次 `path`（`/status`、`/token`）。
async fn ask(port: u16, path: &str) -> serde_json::Value {
    let bearer = format!("Bearer {LOGIN}");
    let answer = get(port, path, &[("Authorization", &bearer)]).await;
    assert_eq!(answer.status, 200, "{path}");
    answer.json()
}

/// 照中文说的那一句。
fn zh(notice: &Notice) -> String {
    Texts::load(ResourceRoot::at(resources()), "zh")
        .expect("读得出来")
        .notice(notice)
}

#[tokio::test]
async fn without_a_token_both_ports_listen_and_napcat_is_refused() {
    let (dir, root) = temp_root();
    let _core = fake_core(&root);
    let bridge = start(serve(root, with_token(None))).await;
    let notices = bridge.notices.lock().expect("没 panic").clone();
    assert!(
        matches!(
            &notices[..],
            [Notice::Listening { port, language }, Notice::NoToken, Notice::Web { port: web }]
                if *port == bridge.port && language == "zh" && *web == bridge.web
        ),
        "先说在哪等 NapCat，再说怎么设令牌：{notices:?}"
    );
    for auth in [Auth::Bearer(TOKEN), Auth::Nothing] {
        let refused = napcat(bridge.port, "/ws", auth, Some(BOT)).await;
        assert_eq!(refused.err(), Some(401), "{auth:?}：端口开着，没令牌一律拒");
    }
    assert_eq!(
        ask(bridge.web, "/status").await,
        json!({"napcat": {"connected": false}, "listen": bridge.port, "web": bridge.web, "token": "none"})
    );
    bridge.stop().await.expect("停得下");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn a_token_set_through_the_core_lets_napcat_in_without_a_restart() {
    let home = Home::new(&Script::new([]));
    let bridge = start(Serve {
        reload: from_disk(&home.root),
        ..serve(home.root.clone(), with_token(None))
    })
    .await;
    let refused = napcat(bridge.port, "/ws", Auth::Bearer(TOKEN), Some(BOT)).await;
    assert_eq!(refused.err(), Some(401), "还没有令牌");
    set_token(&home.root, TOKEN).await;
    admitted(bridge.port, "/ws", Auth::Bearer(TOKEN), Some(BOT))
        .await
        .close()
        .await;
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn a_new_token_takes_over_and_the_open_connection_stays() {
    let home = Home::new(&Script::new([Play::Says("在。")]));
    set_token(&home.root, TOKEN).await;
    let bridge = start(Serve {
        reload: from_disk(&home.root),
        ..serve(home.root.clone(), settings())
    })
    .await;
    let mut open = owner_napcat(bridge.port).await;
    set_token(&home.root, NEW).await;
    // 不报号：不顶掉连着的那一条。
    let fresh = admitted(bridge.port, "/ws", Auth::Bearer(NEW), None).await;
    let old = napcat(
        bridge.port,
        "/onebot/v11/ws",
        Auth::Bearer(TOKEN),
        Some(BOT),
    )
    .await;
    assert_eq!(old.err(), Some(401), "旧的不收");
    open.owner_says(1, "还在吗").await;
    assert_eq!(open.reply().await, "在。", "已经连着的那一条照样收发");
    fresh.close().await;
    bridge.stop().await.expect("停得下");
}

/// 真的程序 `miyu-onebot`：数据根是 `root`，资源目录是源码树的；握手以前说英文（替身握手回中文：说的话换成中文，才看得出
/// 照握手回的语言说）；不用 `$XDG_RUNTIME_DIR`（核心的替身也不用，两头找的是同一个套接字）。
fn program(root: &DataRoot, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_miyu-onebot"));
    command
        .args(args)
        .env("MIYU_HOME", root.path())
        .env("MIYU_RESOURCES", resources())
        .env("LC_ALL", "en_US.UTF-8")
        .env_remove("XDG_RUNTIME_DIR")
        .env_remove("MIYU_LOG");
    command
}

#[tokio::test]
async fn the_program_runs_without_a_token_and_takes_one_without_a_restart() {
    let (dir, root) = temp_root();
    let _core = fake_core(&root);
    let (listen, web) = (free_port(), free_port());
    let system = root.path().join("system");
    std::fs::create_dir_all(&system).expect("建得了");
    let ports = format!("[onebot]\nlisten = {listen}\nweb = {web}\n");
    std::fs::write(system.join("config.toml"), &ports).expect("写得进");
    let mut serve = program(&root, &["serve"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("起得来");
    within("WebUI 开始听", async {
        while TcpStream::connect(("127.0.0.1", web)).await.is_err() {
            if let Some(exited) = serve.try_wait().expect("看得到") {
                let mut said = String::new();
                if let Some(mut stderr) = serve.stderr.take()
                    && stderr.read_to_string(&mut said).is_err()
                {
                    // 读不出来：照空的说。
                }
                panic!("桥退了（{exited}）：{said}");
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert_eq!(
        ask(web, "/status").await,
        json!({"napcat": {"connected": false}, "listen": listen, "web": web, "token": "none"})
    );
    let refused = napcat(listen, "/ws", Auth::Bearer(TOKEN), Some(BOT)).await;
    assert_eq!(refused.err(), Some(401), "NapCat 的端口开着，没令牌一律拒");
    // 照页面写的样子改配置文件：先写引用（密钥还没存），再存密钥。
    let referred = format!("{ports}token = {{ secret = \"onebot\" }}\n");
    std::fs::write(system.join("config.toml"), referred).expect("写得进");
    assert_eq!(ask(web, "/status").await["token"], "missing");
    std::fs::write(
        system.join("secrets.toml"),
        format!("onebot = \"{TOKEN}\"\n"),
    )
    .expect("写得进");
    let connected = admitted(listen, "/ws", Auth::Bearer(TOKEN), Some(BOT)).await;
    assert_eq!(ask(web, "/status").await["token"], "set");
    assert_eq!(ask(web, "/token").await, json!({"token": TOKEN}));
    connected.close().await;
    let opened = tokio::task::spawn_blocking({
        let mut web = program(&root, &["web", "--print"]);
        move || web.output()
    })
    .await
    .expect("没崩")
    .expect("跑得了");
    assert_eq!(
        opened.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&opened.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&opened.stdout),
        format!("http://127.0.0.1:{web}/#setup={CODE}\n"),
        "照常打开"
    );
    serve.kill().expect("停得下");
    let served = tokio::task::spawn_blocking(move || serve.wait_with_output())
        .await
        .expect("没崩")
        .expect("等得到");
    let said = String::from_utf8_lossy(&served.stderr).to_string();
    let listening = zh(&Notice::Listening {
        port: listen,
        language: "zh".to_string(),
    });
    let first = format!(
        "{listening}\n{}\n{}\n",
        zh(&no_token()),
        zh(&Notice::Web { port: web })
    );
    assert!(
        said.starts_with(&first),
        "标准错误上先说在哪等 NapCat，再说怎么设令牌、网页在哪：{said}"
    );
    let log = std::fs::read_to_string(root.state().join("logs").join("onebot.log"))
        .expect("写了运行日志");
    assert!(log.contains("no token yet"), "{log}");
    assert!(log.contains("web token read"), "取过令牌记一行：{log}");
    assert!(!log.contains(TOKEN), "运行日志里没有令牌的值：{log}");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

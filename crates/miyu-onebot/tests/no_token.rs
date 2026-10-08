//! 令牌没设（施工 O-16 补、补二，`onebot.md` 第一条「怎么走」第 1、2 条、「施工时定的」第 3 条，第二条「施工时定的」第 14、
//! 18 条；`18-通讯平台.md` 第三节「还没配好就 `start`：桥只开 WebUI，不连 NapCat，等配好」）：桥照样起来，两个端口都开，说一句
//! 怎么办；NapCat 连进来 401。核心推来令牌（施工 O-20：页面写进令牌，核心换上新配置就推），不重启，NapCat 下一次连就通；再换
//! 一个，新的进得来、旧的不收，已经连着的那一条照样收发。真的程序 `miyu-onebot serve` 照常起来（施工 O-18 起测试当核心，经它的
//! 标准输入输出握手、推送；标准错误、运行日志各一句），推来令牌，`/status` 的 `token` 跟着变、NapCat 不用重启就连上，`/token`
//! 交出值、运行日志里没有它；`miyu-onebot web --print` 照状态文件里的端口印网址。

use serde_json::json;

use miyu_onebot::serve::Notice;
use miyu_onebot::texts::Texts;
use miyu_session::testkit::{Play, Script};
use miyu_store::resources::ResourceRoot;

use crate::support::fake_core::{CODE, LOGIN, fake_core};
use crate::support::http::*;
use crate::support::ports::on_free_ports;
use crate::support::spawning::{program, served_up};
use crate::support::*;

/// 换上的新令牌。
const NEW: &str = "napcat-new-token";

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
            [Notice::Listening { port }, Notice::NoToken, Notice::Web { port: web }]
                if *port == bridge.port && *web == bridge.web
        ),
        "先说在哪等 NapCat，再说怎么设令牌：{notices:?}"
    );
    for auth in [Auth::Bearer(TOKEN), Auth::Nothing] {
        let refused = napcat(bridge.port, "/ws", auth, Some(BOT)).await;
        assert_eq!(refused.err(), Some(401), "{auth:?}：端口开着，没令牌一律拒");
    }
    assert_eq!(
        ask(bridge.web, "/status").await,
        json!({"napcat": {"connected": false}, "listen": bridge.port, "web": bridge.web, "token": "none", "platform": "qq"})
    );
    bridge.stop().await.expect("停得下");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn a_pushed_token_lets_napcat_in_without_a_restart() {
    let home = Home::new(&Script::new([]));
    let (serve, push) = serve_pushing(home.root.clone(), with_token(None));
    let bridge = start(serve).await;
    let refused = napcat(bridge.port, "/ws", Auth::Bearer(TOKEN), Some(BOT)).await;
    assert_eq!(refused.err(), Some(401), "还没有令牌");
    push.config(json!({"onebot.token": TOKEN}));
    admitted(bridge.port, "/ws", Auth::Bearer(TOKEN), Some(BOT))
        .await
        .close()
        .await;
    bridge.stop().await.expect("停得下");
}

#[tokio::test]
async fn a_new_token_takes_over_and_the_open_connection_stays() {
    let home = Home::new(&Script::new([Play::Says("在。")]));
    let (serve, push) = serve_pushing(home.root.clone(), settings());
    let bridge = start(serve).await;
    let mut open = owner_napcat(bridge.port).await;
    push.config(json!({"onebot.token": NEW}));
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

#[tokio::test]
async fn the_program_runs_without_a_token_and_takes_one_without_a_restart() {
    let (dir, root) = temp_root();
    let _core = fake_core(&root);
    // 挑的空端口被别人先占了的换一组再来（`support/ports.rs`）；回来的时候两个端口都听上了。握手交两个端口、没有令牌。
    let (mut serve, listen, web) =
        on_free_ports(async |listen, web| Ok((served_up(&root, listen, web).await?, listen, web)))
            .await;
    assert_eq!(
        ask(web, "/status").await,
        json!({"napcat": {"connected": false}, "listen": listen, "web": web, "token": "none", "platform": "qq"})
    );
    let refused = napcat(listen, "/ws", Auth::Bearer(TOKEN), Some(BOT)).await;
    assert_eq!(refused.err(), Some(401), "NapCat 的端口开着，没令牌一律拒");
    // 照核心的样子推来令牌（页面写进令牌，核心换上新配置就推，施工 O-20）。
    serve.config(json!({"onebot.token": TOKEN})).await;
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
    // 核心请它退出：关它的标准输入（施工 O-18）。
    drop(serve.stdin.take());
    let served = within("桥退出", serve.child.wait_with_output())
        .await
        .expect("等得到");
    assert_eq!(served.status.code(), Some(0), "读到头好好停下");
    let said = String::from_utf8_lossy(&served.stderr).to_string();
    let listening = zh(&Notice::Listening { port: listen });
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

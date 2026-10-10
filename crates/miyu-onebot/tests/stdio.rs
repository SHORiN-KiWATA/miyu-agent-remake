//! 真的程序 `serve` 经标准输入输出说协议（施工 O-18，`onebot.md` 第一条「对外的样子」「怎么走」第 1、11 条）：测试当核心。
//! 握手报 `onebot`、不带凭据；标准输出上只有协议，说给人听的在标准错误上；关了标准输入（核心请它退出）5 秒内退出、退出码 0，
//! 运行日志记一行。`-h`、`--help` 印用法到标准输出、退出码 0；`web` 后面多带的照用法不对（施工 O-28 补）。握手交的端口
//! 被占：握手以前不说话，被占那一句照握手回的语言说（施工 O-20）。握手交来 `onebot.web` 的不认：只开 NapCat 的端口，状态文件
//! 里没有 `web`，标准错误上不说网页（施工 O-28 下）。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_onebot::serve::{Failure, Notice};
use miyu_onebot::texts::Texts;
use miyu_store::resources::ResourceRoot;

use crate::support::ports::on_free_port;
use crate::support::spawning::{free_port, program, served, served_up, served_with, text};
use crate::support::*;

#[tokio::test]
async fn serve_speaks_only_the_protocol_on_stdout_and_stops_when_stdin_ends() {
    let (dir, root) = temp_root();
    // 挑的空端口被别人先占了的换一个再来（`support/ports.rs`）；回来的时候端口听上了。端口由握手交（施工 O-20）。
    let (mut served, listen) =
        on_free_port(async |listen| Ok((served_up(&root, listen).await?, listen))).await;
    let hello = &served.hello;
    assert_eq!(hello["method"], "hello", "{hello}");
    assert_eq!(hello["params"]["head"]["kind"], "onebot", "{hello}");
    assert_eq!(hello["params"]["caps"], json!({"input": false}), "{hello}");
    for credential in ["token", "code", "login", "user", "password"] {
        assert!(
            hello["params"].get(credential).is_none(),
            "标准输入输出是核心亲手给的，不带凭据：{hello}"
        );
    }
    // 核心请它退出：关它的标准输入。
    drop(served.stdin.take());
    let exited = tokio::time::timeout(Duration::from_secs(5), served.child.wait_with_output())
        .await
        .expect("读到头 5 秒内退出")
        .expect("等得到");
    assert_eq!(exited.status.code(), Some(0), "{}", text(&exited.stderr));
    let lines = within("标准输出读到头", served.stdout).await.expect("没崩");
    // 握手、登记工具（施工 O-26）、登记后台页的方法（施工 O-28 上）三行：起来、停下都不往标准输出写别的。
    assert_eq!(lines.len(), 3, "起来、停下都不往标准输出写别的：{lines:?}");
    for line in &lines {
        let message: Value = serde_json::from_str(line).expect("每一行都是 JSON");
        assert_eq!(message["jsonrpc"], "2.0", "{line}");
        assert!(message["method"].is_string(), "是请求：{line}");
    }
    let provided: Value = serde_json::from_str(&lines[1]).expect("是 JSON");
    assert_eq!(provided["method"], "provide", "{provided}");
    let registered: Value = serde_json::from_str(&lines[2]).expect("是 JSON");
    assert_eq!(registered["method"], "package.methods", "{registered}");
    let said = text(&exited.stderr);
    let zh = Texts::load(ResourceRoot::at(resources()), "zh").expect("读得出来");
    let listening = zh.notice(&Notice::Listening { port: listen });
    assert!(
        said.starts_with(&listening),
        "说给人听的在标准错误上：{said}"
    );
    let log =
        std::fs::read_to_string(root.state().join("logs").join("onebot.log")).expect("有运行日志");
    assert!(log.contains("core closed, stopping"), "{log}");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn a_port_in_use_is_said_in_the_language_of_the_handshake() {
    let (dir, root) = temp_root();
    // 占着的端口由测试自己从系统挑来、一直拿着；桥绑不上就退。
    let taken = std::net::TcpListener::bind("127.0.0.1:0").expect("挑得到");
    let port = taken.local_addr().expect("有地址").port();
    // 系统的语言是英文（`program`），握手回中文：照中文说才是照握手回的语言。
    let served = served(&root, json!({"onebot.listen": port})).await;
    let exited = within("桥退出", served.child.wait_with_output())
        .await
        .expect("等得到");
    assert_eq!(exited.status.code(), Some(1), "{}", text(&exited.stderr));
    let zh = Texts::load(ResourceRoot::at(resources()), "zh").expect("读得出来");
    assert_eq!(
        text(&exited.stderr),
        format!("{}\n", zh.failure(&Failure::PortInUse(port))),
        "握手以前不说话，被占的那一句照握手回的语言"
    );
    drop(taken);
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn help_goes_to_stdout_and_a_wrong_word_is_a_usage_error() {
    let (dir, root) = temp_root();
    let en = Texts::load(ResourceRoot::at(resources()), "en").expect("读得出来");
    for flag in ["-h", "--help"] {
        let mut command = program(&root, &[flag]);
        let shown = tokio::task::spawn_blocking(move || command.output())
            .await
            .expect("没崩")
            .expect("跑得了");
        assert_eq!(shown.status.code(), Some(0), "{flag}");
        assert_eq!(text(&shown.stdout), format!("{}\n", en.usage()), "{flag}");
        assert_eq!(text(&shown.stderr), "", "{flag}");
    }
    for wrong in [
        &["nonsense"][..],
        &["logs", "-x"],
        &["start", "now"],
        &[],
        &["web", "--print"],
    ] {
        let mut command = program(&root, wrong);
        let refused = tokio::task::spawn_blocking(move || command.output())
            .await
            .expect("没崩")
            .expect("跑得了");
        assert_eq!(refused.status.code(), Some(2), "{wrong:?}");
        assert_eq!(
            text(&refused.stderr),
            format!("{}\n", en.usage()),
            "{wrong:?}"
        );
        assert_eq!(text(&refused.stdout), "", "{wrong:?}");
    }
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn a_web_port_in_the_handshake_is_not_opened() {
    let (dir, root) = temp_root();
    // 原来桥自己的网页的端口：握手交来了也不认（施工 O-28 下，「施工时定的」第 170 条）。
    let web = free_port();
    let (mut served, listen) = on_free_port(async |listen| {
        let config = json!({"onebot.listen": listen, "onebot.web": web, "onebot.token": TOKEN});
        Ok((served_with(&root, config, listen).await?, listen))
    })
    .await;
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", web))
            .await
            .is_err(),
        "只开 NapCat 的端口"
    );
    let board = miyu_onebot::status_file::read(&root).expect("写了状态文件");
    assert_eq!(board["listen"], listen, "{board}");
    assert!(
        board.get("web").is_none(),
        "状态文件里没有网页的端口：{board}"
    );
    drop(served.stdin.take());
    let exited = within("桥退出", served.child.wait_with_output())
        .await
        .expect("等得到");
    assert_eq!(exited.status.code(), Some(0), "{}", text(&exited.stderr));
    let zh = Texts::load(ResourceRoot::at(resources()), "zh").expect("读得出来");
    assert_eq!(
        text(&exited.stderr),
        format!("{}\n", zh.notice(&Notice::Listening { port: listen })),
        "标准错误上只说在哪等 NapCat，不说网页"
    );
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

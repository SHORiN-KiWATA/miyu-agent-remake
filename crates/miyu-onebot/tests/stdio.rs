//! 真的程序 `serve` 经标准输入输出说协议（施工 O-18，`onebot.md` 第一条「对外的样子」「怎么走」第 1、11 条）：测试当核心。
//! 握手报 `onebot`、不带凭据；标准输出上只有协议，说给人听的在标准错误上；关了标准输入（核心请它退出）5 秒内退出、退出码 0，
//! 运行日志记一行。`-h`、`--help` 印用法到标准输出、退出码 0。握手交的端口被占：握手以前不说话，被占那一句照握手回的
//! 语言说（施工 O-20）。

use std::time::Duration;

use serde_json::{Value, json};

use miyu_onebot::serve::{Failure, Notice};
use miyu_onebot::texts::Texts;
use miyu_store::resources::ResourceRoot;

use crate::support::ports::on_free_ports;
use crate::support::spawning::{program, served, served_up, text};
use crate::support::*;

#[tokio::test]
async fn serve_speaks_only_the_protocol_on_stdout_and_stops_when_stdin_ends() {
    let (dir, root) = temp_root();
    // 挑的空端口被别人先占了的换一组再来（`support/ports.rs`）；回来的时候两个端口都听上了。两个端口由握手交（施工 O-20）。
    let (mut served, listen) =
        on_free_ports(async |listen, web| Ok((served_up(&root, listen, web).await?, listen))).await;
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
    // 占着的端口由测试自己从系统挑来、一直拿着；桥先绑 NapCat 的这一个，绑不上就退，WebUI 的用不上。
    let taken = std::net::TcpListener::bind("127.0.0.1:0").expect("挑得到");
    let port = taken.local_addr().expect("有地址").port();
    // 系统的语言是英文（`program`），握手回中文：照中文说才是照握手回的语言。
    let served = served(&root, json!({"onebot.listen": port, "onebot.web": 0})).await;
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
    for wrong in [&["nonsense"][..], &["logs", "-x"], &["start", "now"], &[]] {
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

//! 跟核心的管道（施工 O-18，`onebot.md` 第一条「怎么走」第 1、11 条，「施工时定的」第 21、22 条）：桥在给的管道上握手，报
//! `onebot`、不带凭据；核心关了管道（读到头），桥好好停下；`hello_seconds` 里等不到握手的回应，说 `serve` 只由核心拉起。
//! 测试当核心，管道是内存里的。

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use miyu_onebot::serve::{Failure, Pipe, Serve, run};

use crate::support::*;

#[tokio::test]
async fn when_the_core_closes_the_pipe_the_bridge_stops_quietly() {
    let (dir, root) = temp_root();
    let (bridge_end, core_end) = tokio::io::duplex(1 << 16);
    let (read, write) = tokio::io::split(bridge_end);
    let core = tokio::spawn(async move {
        let (reader, mut writer) = tokio::io::split(core_end);
        let mut lines = BufReader::new(reader).lines();
        let line = lines.next_line().await.expect("读得了").expect("有一行");
        let hello: Value = serde_json::from_str(&line).expect("是 JSON");
        // 照核心拉起扩展的样子交两个端口（施工 O-20）：0 让系统挑，不碰出厂的 8301、8302。
        let config = json!({"onebot.listen": 0, "onebot.web": 0});
        let reply = json!({"jsonrpc": "2.0", "id": hello["id"], "result": {"protocol": 1, "language": "zh", "config": config}});
        writer
            .write_all(format!("{reply}\n").as_bytes())
            .await
            .expect("写得出");
        (hello, lines, writer)
    });
    let bridge = start(Serve {
        pipe: Pipe::new(read, write),
        ..serve(root, settings())
    })
    .await;
    let (hello, lines, writer) = within("握手", core).await.expect("没崩");
    assert_eq!(hello["method"], "hello", "{hello}");
    assert_eq!(hello["params"]["head"]["kind"], "onebot", "{hello}");
    assert_eq!(hello["params"]["protocol"], json!([1, 1]), "{hello}");
    for credential in ["token", "code", "login", "user", "password"] {
        assert!(
            hello["params"].get(credential).is_none(),
            "核心亲手给的管道，不带凭据：{hello}"
        );
    }
    // 核心请它退出：关上管道。
    drop((lines, writer));
    assert_eq!(bridge.ended().await, Ok(()), "读到头好好停下，不当出错");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn without_a_hello_reply_it_says_the_core_starts_it() {
    let (dir, root) = temp_root();
    let (bridge_end, core_end) = tokio::io::duplex(1 << 16);
    let (read, write) = tokio::io::split(bridge_end);
    let mut tuning = tuning();
    tuning.hello_seconds = 1;
    let serve = Serve {
        pipe: Pipe::new(read, write),
        tuning,
        ..serve(root, settings())
    };
    // 核心那一头开着、一直不回：照从终端跑起来的样子。
    let ran = within("桥退出", run(serve, |_| {}, |_| {}, std::future::pending())).await;
    drop(core_end);
    assert_eq!(ran, Err(Failure::NotSpawned));
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn a_pipe_closed_during_hello_is_a_failure_to_reach_the_core() {
    let (dir, root) = temp_root();
    let (bridge_end, core_end) = tokio::io::duplex(1 << 16);
    let (read, write) = tokio::io::split(bridge_end);
    drop(core_end);
    let serve = Serve {
        pipe: Pipe::new(read, write),
        ..serve(root, settings())
    };
    let ran = within("桥退出", run(serve, |_| {}, |_| {}, std::future::pending())).await;
    assert!(matches!(ran, Err(Failure::Core(_))), "{ran:?}");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

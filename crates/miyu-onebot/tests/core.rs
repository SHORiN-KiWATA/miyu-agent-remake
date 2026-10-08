//! 跟核心的那一头（施工 O-8，`onebot.md` 第一条「怎么走」第 1 条）：握手的回应一定带 `language`（`protocol.md`「握手」）；
//! 没带的是协议不对，照「连不上核心」说清、退出，不悄悄当成英文接着跑。

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use miyu_onebot::serve::{Failure, Serve, run};

use crate::support::*;

/// 假的核心：接一条连接，`hello` 照 `result` 回，之后留着连接直到桥断开。
async fn fake_core(mut listener: miyu_ipc::Listener, result: Value) {
    let connection = listener.accept().await.expect("连进来了");
    let (reader, mut writer) = tokio::io::split(connection);
    let mut lines = BufReader::new(reader).lines();
    let line = lines.next_line().await.expect("读得了").expect("有一行");
    let hello: Value = serde_json::from_str(&line).expect("是 JSON");
    assert_eq!(hello["method"], "hello", "{hello}");
    let reply = json!({"jsonrpc": "2.0", "id": hello["id"], "result": result});
    writer
        .write_all(format!("{reply}\n").as_bytes())
        .await
        .expect("写得出");
    while let Ok(Some(_)) = lines.next_line().await {}
}

#[tokio::test]
async fn a_hello_reply_without_a_language_is_a_failure_to_reach_the_core() {
    let (dir, root) = temp_root();
    let dirs = miyu_ipc::Dirs {
        runtime_dir: None,
        ..miyu_ipc::Dirs::current()
    };
    let opened = miyu_ipc::open(&root, &dirs).expect("起得来");
    let core = tokio::spawn(fake_core(opened.listener, json!({"protocol": [1, 1]})));
    let serve = Serve {
        locale: None,
        ..serve(root, settings())
    };
    let ran = within("桥退出", run(serve, |_| {}, |_| {}, std::future::pending())).await;
    core.abort();
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
    match ran {
        Err(Failure::Core(reason)) => assert!(reason.contains("language"), "{reason}"),
        other => panic!("握手没回语言要照连不上核心退：{other:?}"),
    }
}

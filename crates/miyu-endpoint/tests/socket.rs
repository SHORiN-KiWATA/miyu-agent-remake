//! 在真的套接字上（`docs/construction/3-8-协议端点（下）.md` 验收第 2 条；Windows 上是命名管道，施工 3-8 补）：
//! 核心起来，头照 `run/socket`、`run/token` 连上，握手、造会话、说一句，这一轮说完；第二个头也连得上。

use serde_json::json;

use crate::support::{Client, Home, kinds};
use miyu_endpoint::run;
use miyu_ipc::{Dirs, connect, open};
use miyu_session::testkit::{Play, Script};

#[tokio::test]
async fn a_head_talks_to_the_core_over_the_socket() {
    let home = Home::new();
    let dirs = Dirs {
        runtime_dir: None,
        ..Dirs::current()
    };
    let opened = open(&home.root, &dirs).expect("起得来");
    let core = home.core_with(&Script::new([Play::Says("你好。")]), &opened.token);
    let running = tokio::spawn(run(opened.listener, core));

    let (connection, token) = connect(&home.root).await.expect("连得上");
    let mut head = Client::over(connection);
    let reply = head.hello_as(&token).await;
    assert_eq!(reply["result"]["account"], json!("alice"), "{reply}");
    let session = head.create("c1", "/work").await;
    head.subscribe("s1", &session).await;
    let reply = head.say("m1", &session, "在吗").await;
    assert!(reply.get("error").is_none(), "{reply}");
    let pushed = head.until_turn_ends(&session).await;
    assert!(
        kinds(&pushed).contains(&"message.assistant".to_string()),
        "{pushed:?}"
    );

    // 第二个头：核心一个个接下去。
    let (connection, token) = connect(&home.root).await.expect("连得上");
    let mut second = Client::over(connection);
    let reply = second.hello_as(&token).await;
    assert_eq!(reply["result"]["account"], json!("alice"), "{reply}");

    running.abort();
    assert!(running.await.is_err_and(|error| error.is_cancelled()));
    let error = connect(&home.root).await.expect_err("核心停了");
    assert!(
        matches!(error, miyu_ipc::ConnectError::NotRunning),
        "{error:?}"
    );
}

//! `/status`（施工 O-16，`onebot.md` 第二条「对外的样子」「施工时定的」第 3、6 条）：要 `Authorization: Bearer <登录令牌>`，
//! 桥拿它和核心握手验，不带的、带错的 401；验过的记一阵（只记哈希），这一阵里不再握手、过了再握手；回 NapCat 连没连上、
//! 是哪个实现、两个端口、令牌设没设（补二：`token` 的三种由 `no_token.rs` 照配置文件守着）。核心是替身，数得出握了几次手。

use std::time::Duration;

use serde_json::json;

use miyu_onebot::serve::Serve;

use crate::support::fake_core::{LOGIN, fake_core};
use crate::support::http::*;
use crate::support::*;

/// `Authorization: Bearer <token>`。
fn bearer(token: &str) -> [(&'static str, String); 1] {
    [("Authorization", format!("Bearer {token}"))]
}

/// 带着 `token` 取一次 `/status`。
async fn status(port: u16, token: &str) -> Answer {
    let [(name, value)] = bearer(token);
    get(port, "/status", &[(name, &value)]).await
}

#[tokio::test]
async fn without_the_right_login_token_there_is_no_status() {
    let (dir, root) = temp_root();
    let core = fake_core(&root);
    let bridge = start(serve(root, settings())).await;
    let port = bridge.web;
    assert_eq!(get(port, "/status", &[]).await.status, 401, "没带");
    for header in [
        format!("Bearer {}", &LOGIN[1..]),
        format!("Bearer {LOGIN}x"),
        "Bearer ".to_string(),
        "Bearer".to_string(),
        format!("Basic {LOGIN}"),
        format!("Token {LOGIN}"),
    ] {
        let refused = get(port, "/status", &[("Authorization", &header)]).await;
        assert_eq!(refused.status, 401, "{header}");
        assert_eq!(refused.header("x-content-type-options"), Some("nosniff"));
    }
    let answer = status(port, LOGIN).await;
    assert_eq!(answer.status, 200);
    assert_eq!(answer.header("content-type"), Some("application/json"));
    assert_eq!(answer.header("cache-control"), Some("no-store"));
    assert_eq!(
        answer.json(),
        json!({"napcat": {"connected": false}, "listen": bridge.port, "web": port, "token": "set"})
    );
    // 认了的之后再带错的：照样 401，不因为前面有人验过就放过。
    assert_eq!(status(port, "wrong").await.status, 401);
    let host = format!("127.0.0.1:{port}");
    let [(name, value)] = bearer(LOGIN);
    let post = request(port, "POST", "/status", &host, &[(name, &value)]).await;
    assert_eq!(post.status, 405);
    drop(core);
    bridge.stop().await.expect("停得下");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn a_checked_token_is_remembered_for_a_while_only() {
    let (dir, root) = temp_root();
    let core = fake_core(&root);
    let mut tuning = tuning();
    // 记 2 秒（出厂 60 秒）：慢的机器上前面几问也落在这 2 秒里。
    tuning.web.status_cache_seconds = 2;
    let bridge = start(Serve {
        tuning,
        ..serve(root, settings())
    })
    .await;
    let port = bridge.web;
    assert_eq!(status(port, LOGIN).await.status, 200);
    assert_eq!(status(port, LOGIN).await.status, 200);
    assert_eq!(status(port, LOGIN).await.status, 200);
    assert_eq!(core.logins(), 1, "记着的不再握手");
    // 错的不记：每次都握手、每次都 401。
    assert_eq!(status(port, "wrong").await.status, 401);
    assert_eq!(status(port, "wrong").await.status, 401);
    assert_eq!(core.logins(), 3);
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(status(port, LOGIN).await.status, 200);
    assert_eq!(core.logins(), 4, "过了时候再握一次");
    bridge.stop().await.expect("停得下");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn the_status_says_which_napcat_is_connected() {
    let (dir, root) = temp_root();
    let _core = fake_core(&root);
    let bridge = start(serve(root, settings())).await;
    let mut napcat = owner_napcat(bridge.port).await;
    napcat.version().await;
    let napcat_status = within("记下是哪个实现", async {
        loop {
            let answer = status(bridge.web, LOGIN).await.json();
            if answer["napcat"]["version"].is_string() {
                return answer["napcat"].clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    assert_eq!(
        napcat_status,
        json!({"connected": true, "implementation": "NapCat.Onebot", "version": "4.8.0", "self_id": BOT.to_string()})
    );
    napcat.close().await;
    within("断开以后说没连着", async {
        while status(bridge.web, LOGIN).await.json()["napcat"] != json!({"connected": false}) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    bridge.stop().await.expect("停得下");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

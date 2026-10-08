//! `/token`（施工 O-16 补二，`onebot.md` 第二条「对外的样子」「施工时定的」第 17 条）：要登录令牌，和 `/status` 一样；回桥手里的
//! 令牌的值，没写引用、取不到的是 `null`；`no-store`；`GET` 以外 405。核心是替身，重读的配置由测试给。

use std::path::PathBuf;

use serde_json::json;

use miyu_onebot::serve::Serve;
use miyu_onebot::settings::Settings;

use crate::support::fake_core::{FakeCore, LOGIN, fake_core};
use crate::support::http::*;
use crate::support::*;

/// 起一个桥：起来时、重读时读到的都是 `settings`。交回桥、临时目录（用完删）和核心的替身。
async fn bridge_with(settings: Settings) -> (Bridge, PathBuf, FakeCore) {
    let (dir, root) = temp_root();
    let core = fake_core(&root);
    let bridge = start(Serve {
        reload: same(settings.clone()),
        ..serve(root, settings)
    })
    .await;
    (bridge, dir, core)
}

#[tokio::test]
async fn the_token_is_shown_only_with_the_login_token() {
    let (bridge, dir, _core) = bridge_with(settings()).await;
    let port = bridge.web;
    let refused = get(port, "/token", &[]).await;
    assert_eq!(refused.status, 401, "没带");
    assert_eq!(refused.body, b"", "不带值");
    for header in [
        format!("Bearer {LOGIN}x"),
        format!("Bearer {}", &LOGIN[1..]),
        format!("Token {LOGIN}"),
        "Bearer ".to_string(),
    ] {
        let refused = get(port, "/token", &[("Authorization", &header)]).await;
        assert_eq!(refused.status, 401, "{header}");
        assert_eq!(refused.body, b"", "{header}：不带值");
    }
    let bearer = format!("Bearer {LOGIN}");
    let answer = get(port, "/token", &[("Authorization", &bearer)]).await;
    assert_eq!(answer.status, 200);
    assert_eq!(answer.header("content-type"), Some("application/json"));
    assert_eq!(answer.header("cache-control"), Some("no-store"));
    assert_eq!(answer.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(answer.json(), json!({"token": TOKEN}));
    let host = format!("127.0.0.1:{port}");
    let post = request(port, "POST", "/token", &host, &[("Authorization", &bearer)]).await;
    assert_eq!(post.status, 405);
    bridge.stop().await.expect("停得下");
    if std::fs::remove_dir_all(&dir).is_err() {
        // 删不掉就留在临时目录里，不影响测试。
    }
}

#[tokio::test]
async fn without_a_usable_token_there_is_no_value() {
    for settings in [with_token(None), missing_token()] {
        let (bridge, dir, _core) = bridge_with(settings).await;
        let bearer = format!("Bearer {LOGIN}");
        let answer = get(bridge.web, "/token", &[("Authorization", &bearer)]).await;
        assert_eq!(answer.status, 200);
        assert_eq!(answer.json(), json!({"token": null}));
        bridge.stop().await.expect("停得下");
        if std::fs::remove_dir_all(&dir).is_err() {
            // 删不掉就留在临时目录里，不影响测试。
        }
    }
}

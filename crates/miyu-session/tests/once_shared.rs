//! 冷却表两个入口共用（`docs/blueprint/models.md`「怎么走」第十二条第 1 条，施工 8-20）：会话撞了 429 记的冷却，同一个
//! 路由的一次性入口立刻避开那一家；一次性的撞了，会话的下一次请求也避开。
//!
//! 一台假服务器、两家（施工 8-25 起一家一个 key，几个候选只来自池）。会话用钉住的池 `p`，一次性的用另一个钉住的池 `q`，
//! 成员一样、各有各的指针，都从第一家起：避开只能是因为冷却表是一份。

use std::time::Duration;

use serde_json::json;

use crate::support::calling::{asking, bearer, blobs, entry, frozen, keyed, limited};
use crate::support::routing::{configs, hellos, routes, turn};
use crate::support::{Home, ask, say, until_turn_ends, watch};
use miyu_config::secret::Reference;
use miyu_http::testkit::Server;

/// 两家 `a1`、`a2`（照 [`keyed`]，key 都取得到），再加一个成员一样的池 `q` 给一次性的用。
fn two_keys(base_url: &str) -> (String, Vec<(Reference, String)>) {
    let (text, secrets) = keyed(base_url, 2, &[1, 2]);
    let text = text.replace(
        "[models]",
        "[pools.q]\nmodels = [\"a1/m\", \"a2/m\"]\nstrategy = \"pin\"\n\n[models]",
    );
    (text, secrets)
}

/// 借出来的密钥。
fn borrowed(secrets: &[(Reference, String)]) -> Vec<(Reference, &str)> {
    secrets
        .iter()
        .map(|(reference, value)| (reference.clone(), value.as_str()))
        .collect()
}

/// 池里第 `at` 家（从 0 数）的 key 的认证头。
fn key(at: usize) -> Option<String> {
    Some(format!("Bearer sk-{}", at + 1))
}

#[tokio::test]
async fn a_rate_limit_hit_by_a_session_is_avoided_by_a_one_shot_call_at_once() {
    let mut replies = vec![limited()];
    replies.extend(hellos(5));
    let server = Server::start(replies).await;
    let (text, secrets) = two_keys(&server.base_url);
    let mut home = Home::new();
    home.configs = configs(&text, &borrowed(&secrets));
    let routes = routes(json!({}), Duration::from_secs(60));
    let handle = home.create(&routes).await;
    turn(&handle, "cmd-1").await;
    assert_eq!(
        bearer(&server, 0),
        key(0),
        "会话先发给池里的第一家，撞了 429"
    );
    let (_scratch, blobs) = blobs();
    let answered = entry(&routes)
        .call(
            &frozen(&text, &borrowed(&secrets)),
            &blobs,
            asking(Some("@q"), "platform", "one-shot ping"),
        )
        .await;
    assert!(answered.is_ok(), "{answered:?}");
    let received = server.received();
    let mine: Vec<Option<String>> = received
        .iter()
        .filter(|request| String::from_utf8_lossy(&request.body).contains("one-shot ping"))
        .map(|request| request.header("authorization").map(str::to_string))
        .collect();
    assert_eq!(mine, vec![key(1)], "一次性的也从第一家起：在冷却，避开");
}

#[tokio::test]
async fn a_rate_limit_hit_by_a_one_shot_call_is_avoided_by_the_session() {
    let mut replies = vec![limited()];
    replies.extend(hellos(5));
    let server = Server::start(replies).await;
    let (text, secrets) = two_keys(&server.base_url);
    let mut home = Home::new();
    home.configs = configs(&text, &borrowed(&secrets));
    let routes = routes(json!({}), Duration::from_secs(60));
    let handle = home.create(&routes).await;
    let (_scratch, blobs) = blobs();
    let answered = entry(&routes)
        .call(
            &frozen(&text, &borrowed(&secrets)),
            &blobs,
            asking(Some("@q"), "platform", "one-shot ping"),
        )
        .await;
    assert!(answered.is_ok(), "{answered:?}");
    assert_eq!(
        (bearer(&server, 0), bearer(&server, 1)),
        (key(0), key(1)),
        "一次性的撞了 429，当场换"
    );
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("session ping"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    // 会话的主请求是第三个（起标题的在这一轮答完以后）：第一家在冷却，发给另一家。
    assert_eq!(bearer(&server, 2), key(1));
}

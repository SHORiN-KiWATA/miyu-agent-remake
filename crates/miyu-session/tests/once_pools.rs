//! 一次性入口叫池（`docs/blueprint/models.md`「怎么走」第十二条第 4 条、第三条第 6 条，施工 8-20）：钉住的池每次照指针取
//! 一个成员、指针加一（和新造的会话一样），出错当场换下一个成员；轮换的池一次走一个，在冷却的跳过。

use std::time::Duration;

use serde_json::json;

use crate::support::calling::{asking, blobs, entry, frozen, limited};
use crate::support::routing::{hellos, routes};
use miyu_http::testkit::Server;

/// 几家：`a`、`b`、`c` 照先后在 `servers` 上，都不带 key。池 `p` 是 `a/x`、`b/y`、`c/z` 里的前几个，分法 `strategy`。
fn pooled(servers: &[&Server], strategy: &str) -> String {
    let named = [("a", "x"), ("b", "y"), ("c", "z")];
    let mut text = String::new();
    let mut members = Vec::new();
    for ((provider, model), server) in named.iter().zip(servers) {
        text.push_str(&format!(
            "[providers.{provider}]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n",
            server.base_url
        ));
        members.push(format!("\"{provider}/{model}\""));
    }
    text.push_str(&format!(
        "[pools.p]\nmodels = [{}]\nstrategy = \"{strategy}\"\n",
        members.join(", ")
    ));
    text
}

/// 几台服务器各收到了几个请求。
fn counts(servers: &[&Server]) -> Vec<usize> {
    servers
        .iter()
        .map(|server| server.received().len())
        .collect()
}

/// 叫 `times` 次池 `p`，交回每次真发给的 `<供应商>/<模型>`。
async fn call_pool(config: &str, times: usize) -> Vec<String> {
    let config = frozen(config, &[]);
    let routes = routes(json!({}), Duration::from_secs(60));
    let entry = entry(&routes);
    let (_scratch, blobs) = blobs();
    let mut answered = Vec::new();
    for _ in 0..times {
        let answer = entry
            .call(&config, &blobs, asking(Some("@p"), "platform", "hi"))
            .await
            .expect("答得上来");
        answered.push(format!("{}/{}", answer.provider, answer.model));
    }
    answered
}

#[tokio::test]
async fn a_pinned_pool_takes_the_pointer_once_per_call_and_moves_on_to_the_next_member_on_error() {
    let mut replies = vec![limited()];
    replies.extend(hellos(1));
    let (a, b, c) = (
        Server::start(hellos(3)).await,
        Server::start(hellos(2)).await,
        Server::start(replies).await,
    );
    let answered = call_pool(&pooled(&[&a, &b, &c], "pin"), 4).await;
    // 照指针 a、b、c（和新造的会话一样，一次走一个）；c 撞了 429，当场从 c 往下绕到 a，指针不再多走；第四次指针指 a。
    assert_eq!(answered, ["a/x", "b/y", "a/x", "a/x"]);
    assert_eq!(counts(&[&a, &b, &c]), [3, 1, 1]);
}

#[tokio::test]
async fn a_rotating_pool_moves_once_per_call_and_skips_the_one_cooling() {
    let mut replies = hellos(1);
    replies.push(limited());
    replies.extend(hellos(1));
    let (a, b) = (Server::start(replies).await, Server::start(hellos(3)).await);
    let answered = call_pool(&pooled(&[&a, &b], "rotate"), 4).await;
    // a、b、a（429，换的这一次指针又走一个，到 b）、指针又指 a：a 在冷却，跳过、发给 b。
    assert_eq!(answered, ["a/x", "b/y", "b/y", "b/y"]);
    assert_eq!(counts(&[&a, &b]), [2, 3]);
}

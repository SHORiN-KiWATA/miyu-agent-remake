//! 会话的请求记金额（施工 8-15，`docs/blueprint/models.md`「怎么走」第九条第 2、3 条）：路由照真发的那个模型手写的价格、
//! 供应商的倍率算好，`model.called` 带 `cost`，出处写配置那一层和第几行；没价格的不写。

mod support;

use std::sync::Arc;
use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::event::{Body, ModelCalled, Prices, Real};
use support::routing::{configs, routes};
use support::{Home, ask, say, until_turn_ends, watch};

/// 说「你好！」：报 2000 输入（其中 1920 命中）、3 输出。
fn hello() -> Reply {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/designs/samples/drivers/openai-chat/streams/openai-text.sse");
    Reply::stream(vec![Piece::Bytes(
        std::fs::read(&path).expect("样本读得到"),
    )])
}

/// 一家 `dev` 在 `base_url`（不当本机的服务），倍率 0.5，模型 `m` 手写的价格另写 `price` 那几行。
fn config(base_url: &str, price: &str) -> String {
    format!(
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"{base_url}\"\nlocal = false\nprice_multiplier = 0.5\n\n[providers.dev.models.m.price]\n{price}\n[models]\nchat = \"dev/m\"\n"
    )
}

/// 说一轮，交回那一条 `model.called`。
async fn called(price: &str) -> ModelCalled {
    let server = Server::start(vec![hello()]).await;
    let mut home = Home::new();
    home.configs = configs(&config(&server.base_url, price), &[]);
    let routes = Arc::new(routes(serde_json::json!({}), Duration::from_secs(5)));
    let handle = home.create(&*routes).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    tokio::time::timeout(Duration::from_secs(20), until_turn_ends(&mut pushes))
        .await
        .expect("这一轮了结");
    home.log(handle.id())
        .into_iter()
        .find_map(|event| match event.body {
            Body::ModelCalled(called) => Some(called),
            _ => None,
        })
        .expect("记了 model.called")
}

#[tokio::test]
async fn the_route_prices_what_it_sent() {
    let called = called("input = 1.0\noutput = 2.0\ncache_read = 0.25\ncurrency = \"EUR\"\n").await;
    let cost = called.cost.expect("算得出");
    assert_eq!(cost.currency, "EUR");
    assert_eq!(cost.multiplier, Real::new(0.5));
    assert_eq!(
        cost.amount,
        Real::new((80.0 + 1920.0 * 0.25 + 3.0 * 2.0) / 1e6 * 0.5)
    );
    assert_eq!(
        cost.price,
        Prices {
            input: Some(Real::new(1.0)),
            output: Some(Real::new(2.0)),
            cache_read: Some(Real::new(0.25)),
            cache_write: None,
        }
    );
    assert_eq!(
        cost.source, "config:system:8",
        "第一项价格那一行；不认数据根的配置写层的名字"
    );
}

#[tokio::test]
async fn a_used_item_without_a_price_writes_no_cost() {
    // 读缓存用了 1920，却没写读缓存的价。
    let called = called("input = 1.0\noutput = 2.0\n").await;
    assert!(called.usage.is_some());
    assert_eq!(called.cost, None);
}

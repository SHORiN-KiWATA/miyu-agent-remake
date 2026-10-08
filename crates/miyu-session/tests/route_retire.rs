//! 下架的模型（施工 8-23，`docs/construction/8-23-下架的模型移出池（要补）.md`）：池的成员回了 404，后台当场再拉一次这一家的
//! 模型列表，拉成了、里面没有它，才交给端口说它下架了；列表里还有它、拉不成的，交「没下架」。直接写在 `models.chat` 的不查。

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::support::Home;
use crate::support::routing::{configs, hellos, routes_with, turn};
use miyu_http::Proxy;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::time::Timestamp;
use miyu_models::matching::Vendors;
use miyu_models::observed::ProviderList;
use miyu_models::profile::Profiles;
use miyu_session::{ModelData, Observed, Retirement};

/// 端口收到的：哪一家、哪个模型、下架了没有。
#[derive(Default)]
struct Concluded(Mutex<Vec<(String, String, bool)>>);

impl Retirement for Concluded {
    fn concluded(&self, provider: &str, model: &str, gone: bool) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((provider.to_string(), model.to_string(), gone));
    }
}

impl Concluded {
    /// 等到收到一条，交回它：最多 60 秒。
    async fn first(&self) -> (String, String, bool) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        loop {
            let got = self
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .first()
                .cloned();
            if let Some(got) = got {
                return got;
            }
            assert!(tokio::time::Instant::now() < deadline, "端口一条都没收到");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn all(&self) -> Vec<(String, String, bool)> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// 模型不存在：404。
fn gone() -> Reply {
    Reply::error(
        404,
        &[],
        r#"{"error":{"message":"The model `m` does not exist","code":"model_not_found"}}"#,
    )
}

/// 列出 `models` 的回应。
fn listing(models: &[&str]) -> Reply {
    let data: Vec<_> = models
        .iter()
        .map(|id| serde_json::json!({"id": id}))
        .collect();
    Reply::stream(vec![Piece::Bytes(
        serde_json::json!({"object": "list", "data": data})
            .to_string()
            .into_bytes(),
    )])
}

/// 两家：`a` 在 `first`、`b` 在 `second`，都不带 key；钉住的池 `p` 是 `a/m`、`b/m`。`chat` 是 `models.chat`。
fn pooled(first: &Server, second: &Server, chat: &str) -> String {
    format!(
        "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\n\n\
         [pools.p]\nmodels = [\"a/m\", \"b/m\"]\nstrategy = \"pin\"\n\n[models]\nchat = \"{chat}\"\n",
        first.base_url, second.base_url
    )
}

/// 照 `first` 那一台的回应走一轮：会话钉在池里的第一家（`a`）上，交回端口收到的。`stale` 是以前拉过的 `a` 的列表（没有的
/// 是没拉过）。
async fn one_turn(
    first: Vec<Reply>,
    chat: &str,
    stale: Option<&[&str]>,
) -> (Server, Arc<Concluded>) {
    let (first, second) = (Server::start(first).await, Server::start(hellos(4)).await);
    let mut home = Home::new();
    home.configs = configs(&pooled(&first, &second, chat), &[]);
    // 拉列表要客户端：核心里有，测试的路由默认没有。
    let data = ModelData::new(Profiles::default(), Vendors::default(), None)
        .with_fetcher(miyu_http::fetcher(Proxy::Off).expect("造得出客户端"));
    let mut observed = Observed::default();
    if let Some(models) = stale {
        let models = models
            .iter()
            .map(|id| serde_json::from_value(serde_json::json!({"id": id})).expect("读得进"))
            .collect();
        let fetched = Timestamp::parse("2026-10-01T00:00:00.000Z").expect("合写法");
        observed
            .lists
            .insert("a".to_string(), ProviderList { fetched, models });
    }
    data.loaded(None, observed);
    let routes = routes_with(Arc::new(data), Duration::from_secs(60));
    let concluded = Arc::new(Concluded::default());
    routes
        .data
        .on_retirement(Arc::clone(&concluded) as Arc<dyn Retirement>);
    let handle = home.create(&routes).await;
    turn(&handle, "cmd-1").await;
    (first, concluded)
}

#[tokio::test]
async fn a_pool_member_answering_404_and_missing_from_the_list_is_gone() {
    let (first, concluded) = one_turn(vec![gone(), listing(&["other"])], "@p", None).await;
    assert_eq!(
        concluded.first().await,
        ("a".to_string(), "m".to_string(), true)
    );
    let received = first.received();
    assert_eq!(received.len(), 2, "发了一次、拉了一次列表");
    assert_eq!(
        (
            received[1].method.as_str(),
            received[1].path.ends_with("/models")
        ),
        ("GET", true),
        "{:?}",
        received[1].path
    );
}

#[tokio::test]
async fn still_listed_or_no_list_is_not_gone() {
    let (_first, concluded) = one_turn(vec![gone(), listing(&["m", "other"])], "@p", None).await;
    assert_eq!(
        concluded.first().await,
        ("a".to_string(), "m".to_string(), false),
        "列表里还有它"
    );
    // 以前拉过、里面没有它的旧列表还在：这一次拉不成的，不照旧列表算。
    let broken = Reply::error(500, &[], "{}");
    let (_first, concluded) = one_turn(vec![gone(), broken], "@p", Some(&["other"])).await;
    assert_eq!(
        concluded.first().await,
        ("a".to_string(), "m".to_string(), false),
        "列表拉不成"
    );
}

#[tokio::test]
async fn other_errors_and_a_model_written_directly_are_not_checked() {
    let (first, concluded) = one_turn(vec![gone()], "a/m", None).await;
    let limited = Reply::error(429, &[], r#"{"error":{"message":"slow down"}}"#);
    let (second, limited_concluded) = one_turn(vec![limited], "@p", None).await;
    // 一轮说完了、后台要拉的话这时已经起了：再等一会儿，确认一次都没拉。
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(first.received().len(), 1, "直接写着的不拉列表");
    assert!(concluded.all().is_empty());
    assert!(
        second.received().iter().all(|got| got.method != "GET"),
        "不是 404 的不拉列表"
    );
    assert!(limited_concluded.all().is_empty());
}

//! 路由照模型资料（施工 8-7，`docs/blueprint/models.md`「怎么走」第二条）：目录在写了 `ready` 以后才读完，造会话先等它；
//! 窗口、最大输出照目录交给内核；换上的新目录，新会话当场用。

mod support;

use std::sync::Arc;
use std::time::Duration;

use miyu_models::catalog::{Catalog, CatalogSource, Loaded};
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_session::{ModelData, Observed};
use support::Home;
use support::routing::{configs, routes_with};

/// 真目录裁出来的一份（`miyu-models` 的测试也用它）。
fn trimmed() -> Loaded {
    let text = include_str!("../../miyu-models/testdata/models-dev-trimmed.json");
    Loaded {
        catalog: Catalog::parse(text).expect("读得进").catalog,
        source: CatalogSource::Snapshot,
        fetched: "2026-10-01T03:25:54.000Z".to_string(),
    }
}

/// 只配了 key 的 DeepSeek：驱动、地址照档案，资料照目录。
const DEEPSEEK: &str = "[providers.deepseek]\ndriver = \"openai-chat\"\nbase_url = \"http://unused.invalid\"\n\n[models]\nchat = \"deepseek/deepseek-flash\"\n";

fn pending() -> Arc<ModelData> {
    Arc::new(ModelData::new(
        Profiles::default(),
        Vendors::default(),
        None,
    ))
}

#[tokio::test]
async fn a_session_waits_for_the_catalog_and_takes_its_window() {
    let data = pending();
    let routes = routes_with(Arc::clone(&data), Duration::from_secs(60));
    let mut home = Home::new();
    home.configs = configs(DEEPSEEK, &[]);
    let home = Arc::new(home);
    let creating = {
        let (home, routes) = (Arc::clone(&home), routes.clone());
        tokio::spawn(async move { home.create(&routes).await })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(!creating.is_finished(), "目录没读完，造会话等着");
    data.loaded(Some(trimmed()), Observed::default());
    let handle = tokio::time::timeout(Duration::from_secs(10), creating)
        .await
        .expect("读完了就造好")
        .expect("没有 panic");
    assert_eq!(handle.limits().window, Some(1_000_000), "窗口照目录");
}

#[tokio::test]
async fn an_empty_catalog_still_lets_sessions_start() {
    let data = pending();
    data.loaded(None, Observed::default());
    let routes = routes_with(data, Duration::from_secs(60));
    let mut home = Home::new();
    home.configs = configs(DEEPSEEK, &[]);
    let handle = home.create(&routes).await;
    assert_eq!(handle.limits().window, None, "目录读不了：不主动压");
}

#[tokio::test]
async fn a_new_catalog_is_used_by_the_next_session() {
    let data = pending();
    data.loaded(None, Observed::default());
    let routes = routes_with(Arc::clone(&data), Duration::from_secs(60));
    let mut home = Home::new();
    home.configs = configs(DEEPSEEK, &[]);
    let before = home.create(&routes).await;
    data.replace_catalog(trimmed());
    let after = home.create(&routes).await;
    assert_eq!(before.limits().window, None);
    assert_eq!(after.limits().window, Some(1_000_000));
    assert_eq!(
        data.catalog().map(|loaded| loaded.source),
        Some(CatalogSource::Snapshot)
    );
}

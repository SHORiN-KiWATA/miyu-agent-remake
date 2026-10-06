//! 探本机的模型服务（`docs/blueprint/models.md`「怎么走」第七条第 2 条，施工 8-11）：`provider.detect` 的 `local`。
//!
//! 探哪几家由调的一方照目录和档案挑好（`miyu_models::onboard::local_services`：能用、地址在本机的），这里各发一次列模型
//! （驱动的 `models_path()`，不带 key），几家一起发，各等 [`LOCAL_WAIT`]：没回的、回的不是 2xx、读不出模型列表的当没有。
//! 客户端是不走代理的那一个（[`ModelData::local`]）：环境变量里的代理不会自动绕过回环。

use std::time::Duration;

use tokio::task::JoinSet;

use miyu_drivers::openai_chat::Compat;
use miyu_models::onboard::Listed;
use miyu_models::provider::Driver;

use crate::route::lists::{list_models, listing_texts};
use crate::route::shared::ModelData;

/// 一家本机的服务最多等多久。
pub const LOCAL_WAIT: Duration = Duration::from_millis(300);

/// 本机跑着的一家：它，和它列出来的模型名（照字节排、去重）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Running {
    /// 目录、档案里的那一家。
    pub listed: Listed,
    /// 列出来的模型名。
    pub models: Vec<String>,
}

/// 探 `services` 这几家，交回回了的，照给的先后。没有探本机的客户端的一家都不探。
pub async fn find_local(data: &ModelData, services: Vec<Listed>) -> Vec<Running> {
    let (Some(client), Ok(texts)) = (data.local(), listing_texts()) else {
        return Vec::new();
    };
    let mut probes = JoinSet::new();
    for (at, listed) in services.into_iter().enumerate() {
        let (client, texts) = (client.clone(), texts.clone());
        probes.spawn(async move {
            // 照这一家的驱动列模型（施工 8-12）；开关不影响列模型。只探能用的几家，驱动都认得出。
            let kind = listed.driver.as_deref().and_then(Driver::parse)?;
            let driver = kind.build(Compat::default(), texts);
            let base_url = listed.base_url.clone().unwrap_or_default();
            let found = list_models(&client, driver.as_ref(), &base_url, &[], LOCAL_WAIT).await;
            found.ok().map(|models| {
                let mut models: Vec<String> = models.into_iter().map(|model| model.id).collect();
                models.sort_unstable();
                models.dedup();
                (at, Running { listed, models })
            })
        });
    }
    let mut running: Vec<(usize, Running)> =
        probes.join_all().await.into_iter().flatten().collect();
    running.sort_by_key(|(at, _)| *at);
    running.into_iter().map(|(_, running)| running).collect()
}

//! 照意思找记忆的测试共用的（施工 R-5 下挪出来，R-5 补的远程也用）：人记一条、她搜一句、说一轮、读结果、等补上向量，
//! 和接远程要的空的模型资料。

use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::session::Command;
use miyu_models::matching::Vendors;
use miyu_models::profile::Profiles;
use miyu_recall::{MemoryEvent, Saved};
use miyu_session::testkit::Play;
use miyu_session::{Handle, ModelData, Observed};
use miyu_store::recall::Room;
use miyu_tool::Catalog;

use super::{Home, alice, alice_account, id, now, until_logged, within};

/// 空的模型资料（没有档案、没有目录，读完了）：带一个不走代理的客户端，用量记进场地的汇总。
pub fn model_data(home: &Home) -> Arc<ModelData> {
    let data = ModelData::new(Profiles::default(), Vendors::default(), None)
        .with_fetcher(miyu_http::fetcher(miyu_http::Proxy::Off).expect("造得出"));
    data.loaded(None, Observed::default());
    data.keep_ledger(Arc::clone(&home.usage));
    Arc::new(data)
}

/// alice 的软件工程师那一间。
pub fn persona() -> Room {
    Room::persona(&alice_account(), "engineer")
}

/// 人记一条。
pub fn save(home: &Home, text: &str) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let saved = Saved {
        class: "user".into(),
        text: text.into(),
        sources: Vec::new(),
        audience: vec![alice()],
        replaces: None,
        about: None,
    };
    log.append(now(), alice(), None, &MemoryEvent::Saved(saved))
        .expect("记得下");
}

/// 基础的几件和记忆的三件。
pub fn catalog(home: &Home) -> Catalog {
    let mut tools = miyu_basesystem::tools(home.resources.path()).expect("读得出");
    tools.extend(miyu_memory::tools(home.resources.path()).expect("读得出"));
    Catalog::new(tools).expect("合写法")
}

/// 她调一次 `memory_search`。
pub fn search(query: &str) -> Play {
    let args = serde_json::json!({ "query": query }).to_string();
    Play::calls(&[("memory_search", &args)])
}

/// 说 `words`，等到结束了 `turns` 轮，交回日志。
pub async fn chat(home: &Home, handle: &Handle, turns: usize, words: &str) -> Vec<Event> {
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: None,
    };
    within(
        "回应",
        handle.command(id(&format!("cmd-{turns}")), alice(), said),
    )
    .await
    .expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= turns
    })
    .await
}

/// 日志里最后一次调用的结果。
pub fn last_result(log: &[Event]) -> String {
    let result = log
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一次调用");
    match result.blocks.as_slice() {
        [Block::Text(Text { text })] => text.clone(),
        other => panic!("一段字：{other:?}"),
    }
}

/// 这一份库里键是 `key` 的那一条补上了这个模型的向量没有。
pub fn has_vector(index: &miyu_store::recall::RecallIndex, model: &str, key: &str) -> bool {
    let missing = index.missing(model, 0, 1000).expect("读得了");
    !missing.iter().any(|(_, missing, _)| missing == key)
        && index.keys().expect("读得了").iter().any(|have| have == key)
}

/// 等记忆库里的 `key`（`memory`）或者回合库里以 `key` 开头的那一条（不是 `memory`）补上向量（最多 30 秒）。补是搜的时候起
/// 的：起的时候已经在库里的都补，之后才放进去的（这一轮自己的回合）等下一次搜。
pub async fn filled(home: &Home, model: &str, memory: bool, key: &str) {
    for _ in 0..600 {
        let done = if memory {
            let (log, _) = home.logs.open(&persona()).expect("开得了");
            has_vector(log.index(), model, key)
        } else {
            let (turns, _) = home.recall.turns(&persona());
            let keys = turns.keys().expect("读得了");
            keys.iter()
                .filter(|have| have.starts_with(key))
                .all(|have| has_vector(&turns, model, have))
                && keys.iter().any(|have| have.starts_with(key))
        };
        if done {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("三十秒没补上 {key}");
}

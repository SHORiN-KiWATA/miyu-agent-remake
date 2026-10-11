//! 抽取的测试共用的（施工 R-6 上，`docs/blueprint/memory.md` 第六条）：整理记忆的模型是一台假服务器（`org/m`，经一次性入口），
//! 会话的模型照剧本回；闲多久设成一眨眼。交回的候选照 1 到 [`TURNS`] 每一轮都写一条：只有这一段里真有的那几轮留得下，测试
//! 照日志认是哪几轮。

use std::time::Duration;

use miyu_config::secret::Reference;
use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::event::Body;
use miyu_kernel::id::{Seq, SessionId, TurnId};
use miyu_recall::Entry;
use miyu_recall::redact::KeyShapes;
use miyu_session::{ExtractTexts, Extraction, MergeTexts};
use miyu_store::blob::Blobs;

use super::calling::entry;
use super::meaning::persona;
use super::routing::{configs, routes};
use super::{Home, alice_account};

/// 候选写到第几轮。
pub const TURNS: u64 = 80;

/// 一段 OpenAI 兼容的流：正文是 `text`，报一笔用量。
pub fn stream(text: &str) -> Reply {
    let chunk = |choices: serde_json::Value, usage: serde_json::Value| {
        let event = serde_json::json!({"id": "c1", "object": "chat.completion.chunk", "model": "m",
            "choices": choices, "usage": usage});
        format!("data: {event}\n\n")
    };
    let body = [
        chunk(
            serde_json::json!([{"index": 0, "delta": {"role": "assistant", "content": text}, "finish_reason": null}]),
            serde_json::Value::Null,
        ),
        chunk(
            serde_json::json!([]),
            serde_json::json!({"prompt_tokens": 40, "completion_tokens": 9, "total_tokens": 49}),
        ),
        "data: [DONE]\n\n".to_string(),
    ]
    .concat();
    Reply::stream(vec![Piece::Bytes(body.into_bytes())])
}

/// 交回的候选：1 到 [`TURNS`] 每一轮各一条 `text`。
pub fn every_turn(text: &str) -> Reply {
    let memories: Vec<serde_json::Value> = (1..=TURNS)
        .map(|turn| serde_json::json!({"class": "user", "text": text, "turn": turn}))
        .collect();
    stream(&serde_json::json!({ "memories": memories }).to_string())
}

/// 场地接上抽取：整理记忆的照 `server` 上的 `org/m`（key 是 `{ env = "ORG_KEY" }`，值 `sk-org-0123456789abcdef`），`extra`
/// 接在配置后面；闲 100 毫秒就抽。
pub fn organizer(home: &mut Home, server: &Server, extra: &str) {
    organizer_after(home, server, extra, Duration::from_millis(100));
}

/// 同 [`organizer`]，闲 `idle` 才抽。
pub fn organizer_after(home: &mut Home, server: &Server, extra: &str, idle: Duration) {
    let source = format!(
        "[providers.org]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\nkey = {{ env = \"ORG_KEY\" }}\n\n[memory]\norganizer = \"org/m\"\n{extra}",
        server.base_url
    );
    home.configs = configs(
        &source,
        &[(
            Reference::Env("ORG_KEY".to_string()),
            "sk-org-0123456789abcdef",
        )],
    );
    let resources = home.resources.path().to_path_buf();
    let shapes =
        KeyShapes::parse(&home.resources.memory_secrets().expect("读得到")).expect("写法对");
    let extraction = Extraction {
        texts: ExtractTexts::load(&resources).expect("读得到"),
        shapes,
        ask: entry(&routes(serde_json::json!({}), Duration::from_secs(60))),
        blobs: Blobs::new(home.root.blobs(&alice_account())),
        idle: Some(idle),
        merge: MergeTexts::load(&resources).expect("读得到"),
    };
    assert!(home.memory.give_extraction(extraction));
}

/// 假服务器收到第 `count` 个请求（最多等 10 秒）。
pub async fn requested(server: &Server, count: usize) {
    for _ in 0..200 {
        if server.received().len() >= count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "十秒没收到第 {count} 个请求，收到 {}",
        server.received().len()
    );
}

/// 第 `at` 个请求那一条 user 的字。
pub fn asked(server: &Server, at: usize) -> String {
    let body: serde_json::Value =
        serde_json::from_slice(&server.received()[at].body).expect("请求体是 JSON");
    body["messages"]
        .as_array()
        .and_then(|messages| messages.last())
        .and_then(|message| message["content"].as_str().map(str::to_string))
        .unwrap_or_else(|| panic!("一条 user 的字：{body}"))
}

/// 这一间记下的、记忆模块抽出来的几条。
pub fn extracted(home: &Home) -> Vec<Entry> {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.book(|book| {
        book.all()
            .filter(|entry| matches!(&entry.by, miyu_kernel::origin::By::Module(_)))
            .cloned()
            .collect()
    })
}

/// 会话 `session` 抽到了它日志的第几条；还没抽过的没有。
pub fn mark(home: &Home, session: &SessionId) -> Option<Seq> {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.book(|book| book.extracted(session))
}

/// 会话 `session` 日志里结束了的几轮的编号，照先后。
pub fn ended_turns(home: &Home, session: &SessionId) -> Vec<TurnId> {
    home.log(session)
        .iter()
        .filter(|event| matches!(event.body, Body::TurnEnded(_)))
        .filter_map(|event| event.turn)
        .collect()
}

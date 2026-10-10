//! 常驻的那一块交摘要（施工 R-7 下，`docs/blueprint/memory.md` 第三条第 2 款、第七条第 8 款）：真会话、真记忆日志。合并写过
//! 摘要的，头一轮那一块是摘要一行加上次合并以后新记的（新的在前，合并自己改出来的不在），`refs` 只算列出来的；截满了说还有
//! 几条；摘要合进去的一条后来作废了，摘要不算了，照只列条目；没有摘要的照旧（`memory_summary.rs`）。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_kernel::origin::By;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::time::Timestamp;
use miyu_recall::{MemoryEvent, MemoryId, Merged, Retired, Saved, Summary};
use miyu_session::testkit::{Play, Script};
use miyu_store::recall::Room;

use crate::support::*;

fn persona() -> Room {
    Room::persona(&alice_account(), "engineer")
}

/// 记下的时刻：会话的时区（东九区）里是 10 月 8 日。
fn saved_at() -> Timestamp {
    Timestamp::parse("2026-10-07T20:00:00.000Z").expect("合写法")
}

fn memory_module() -> By {
    serde_json::from_str(r#"{"kind":"module","id":"memory"}"#).expect("合写法")
}

/// 往这一间追加一条，交回编号。
fn append(home: &Home, by: By, event: MemoryEvent) -> MemoryId {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.append(saved_at(), by, None, &event).expect("记得下").id
}

/// 人记一条（`replaces` 是改哪一条）。
fn save(home: &Home, text: &str, by: By, replaces: Option<MemoryId>) -> MemoryId {
    append(
        home,
        by,
        MemoryEvent::Saved(Saved {
            class: "user".into(),
            text: text.into(),
            sources: Vec::new(),
            audience: vec![alice()],
            replaces,
            about: None,
        }),
    )
}

/// 记一次合并：合并改了 `revise`、写了摘要、合到 `upto`。
fn merged(home: &Home, revise: MemoryId, upto: MemoryId) -> MemoryId {
    merged_with(home, revise, upto, "用户养猫，住在东京。")
}

/// 同 [`merged`]，摘要是 `text`。
fn merged_with(home: &Home, revise: MemoryId, upto: MemoryId, text: &str) -> MemoryId {
    let revised = save(home, "改过的旧的", memory_module(), Some(revise));
    append(
        home,
        memory_module(),
        MemoryEvent::Summary(Summary {
            text: text.into(),
            upto: upto.seq(),
        }),
    );
    append(
        home,
        memory_module(),
        MemoryEvent::Merged(Merged {
            upto: upto.seq(),
            given: 2,
            revised: 1,
            retired: 0,
            failed: false,
        }),
    );
    revised
}

fn line(id: MemoryId, text: &str) -> String {
    format!("{id} user 2026-10-08: {text}")
}

/// 一次请求里记忆那一块。
fn summaries(request: &Request) -> Vec<String> {
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks } => blocks.clone(),
            _ => Vec::new(),
        })
        .filter_map(|block| match block {
            Block::Text(Text { text }) if text.starts_with("<memories>") => Some(text),
            _ => None,
        })
        .collect()
}

/// 说一句，交回那一次请求里记忆那一块和日志里记下的 `refs`。
async fn first_block(home: &Home) -> (String, Vec<String>) {
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("在吗")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let block = summaries(&script.requests()[0].1)
        .into_iter()
        .next()
        .expect("交了那一块");
    let refs = home
        .log(handle.id())
        .iter()
        .find_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "memory" => {
                Some(fact.refs.clone())
            }
            _ => None,
        })
        .expect("记下了");
    stop(&handle).await;
    (block, refs)
}

#[tokio::test]
async fn with_a_summary_the_block_is_the_summary_and_what_came_after() {
    let home = Home::new();
    let cat = save(&home, "用户养了一只猫", alice(), None);
    let old = save(&home, "用户住在大阪", alice(), None);
    merged(&home, cat, old);
    let tokyo = save(&home, "用户搬到了东京", alice(), None);
    let rust = save(&home, "用户周末写 Rust", alice(), None);
    // 人自己改的照列：只有合并改出来的不列。
    let go = save(&home, "用户周末写 Rust 和 Go", alice(), Some(rust));
    let (block, refs) = first_block(&home).await;
    assert_eq!(
        block,
        format!(
            "<memories>\nAbout the user: 用户养猫，住在东京。\n{}\n{}\n</memories>\n",
            line(go, "用户周末写 Rust 和 Go"),
            line(tokyo, "用户搬到了东京")
        ),
        "摘要一行在前，之后新记的新的在前；合并改出来的、合进摘要的不在"
    );
    assert_eq!(refs, [go.to_string(), tokyo.to_string()], "refs 不算摘要");
}

#[tokio::test]
async fn only_the_summary_when_nothing_came_after() {
    let home = Home::new();
    let cat = save(&home, "用户养了一只猫", alice(), None);
    merged(&home, cat, cat);
    let (block, refs) = first_block(&home).await;
    assert_eq!(
        block,
        "<memories>\nAbout the user: 用户养猫，住在东京。\n</memories>\n"
    );
    assert!(refs.is_empty());
}

#[tokio::test]
async fn a_full_block_cuts_what_came_after_and_says_how_many() {
    let home = Home::new();
    let cat = save(&home, "用户养了一只猫", alice(), None);
    // 摘要一千五百字节上下：截的时候它也占地方。
    merged_with(&home, cat, cat, &"用户养了一只橘猫。".repeat(55));
    for n in 0..40 {
        save(
            &home,
            &format!("第 {n} 条：{}", "很长的一句话".repeat(5)),
            alice(),
            None,
        );
    }
    let (block, refs) = first_block(&home).await;
    assert!(block.len() <= 3000, "{}", block.len());
    assert!(block.starts_with("<memories>\nAbout the user: "), "{block}");
    assert!(
        block.ends_with(&format!(
            "…and {} more; memory_search finds them.\n</memories>\n",
            40 - refs.len()
        )),
        "{block}"
    );
    assert!(
        block.contains("第 39 条") && !block.contains("第 0 条"),
        "新的在前"
    );
}

#[tokio::test]
async fn once_something_it_covered_is_forgotten_the_summary_is_not_given() {
    let home = Home::new();
    let cat = save(&home, "用户养了一只猫", alice(), None);
    let osaka = save(&home, "用户住在大阪", alice(), None);
    let revised = merged(&home, cat, osaka);
    append(
        &home,
        alice(),
        MemoryEvent::Retired(Retired {
            id: osaka,
            why: "忘掉".into(),
        }),
    );
    let (block, refs) = first_block(&home).await;
    assert!(!block.contains("About the user"), "摘要不算了：{block}");
    assert!(!block.contains("大阪"), "忘了就是忘了：{block}");
    assert_eq!(refs, [revised.to_string()], "照只列条目");
}

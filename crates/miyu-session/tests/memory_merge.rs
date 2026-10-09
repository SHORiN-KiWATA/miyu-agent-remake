//! 合并（施工 R-7 上，`docs/blueprint/memory.md` 第七条）：真会话、真记忆日志；整理记忆的模型是一台假服务器（`org/m`），先交
//! 抽取的回答、再交合并的。会话的模型照剧本回。
//!
//! 够会话的抽完就合：一条 user，指令、现在的摘要（还没有）、新记的；改的指着旧的、类出处听众照旧，作废的带为什么，摘要和合到哪
//! 都记下。不够会话的不合，够了再合；合过的时间没到不再合；相关的旧记忆跟着交、作废的和听众不合的不交；放不下的分两次，合并
//! 自己改出来的不算新记的；同一批连着三次不成的记失败的记号、跳过。

use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::id::Seq;
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_recall::{Entry, MemoryEvent, MemoryId, Merged, Retired, Saved};
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat, persona, save};
use crate::support::*;

fn says(n: usize) -> Script {
    Script::new((0..n).map(|_| Play::Says("好。")))
}

/// 多备的一个回答：假服务器只收备了回答的那几个请求，「不再发」要多备一个才看得出来。
fn spare() -> miyu_http::testkit::Reply {
    stream("{}")
}

/// 合并交回的一段：`revised`、`retired`、`summary` 照写的。
fn merged(json: serde_json::Value) -> miyu_http::testkit::Reply {
    stream(&json.to_string())
}

async fn session(home: &Home, turns: usize) -> Handle {
    home.create_full(
        &says(turns),
        &catalog(home),
        Opening::default(),
        Lines::default(),
    )
    .await
}

/// 这一间的底账：照它看合了没有。
fn book<R>(home: &Home, read: impl FnOnce(&miyu_recall::MemoryBook) -> R) -> R {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    log.book(read)
}

/// 等合并记下（最多 10 秒）。
async fn until_merged(home: &Home) {
    for _ in 0..200 {
        if book(home, |book| book.merged()).is_some() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("十秒没合完");
}

fn entry(home: &Home, id: MemoryId) -> Entry {
    book(home, |book| book.get(id).cloned()).expect("有这一条")
}

/// `dir` 下记忆日志的每一行（照文件名排）。
fn log_lines(dir: &std::path::Path) -> Vec<String> {
    let mut files: Vec<_> = walk(dir);
    files.sort();
    files
        .iter()
        .flat_map(|file| {
            std::fs::read_to_string(file)
                .unwrap_or_default()
                .lines()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.flatten()
        .flat_map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                walk(&path)
            } else if path.extension().is_some_and(|ext| ext == "jsonl") {
                vec![path]
            } else {
                Vec::new()
            }
        })
        .collect()
}

fn id(n: u64) -> MemoryId {
    MemoryId::new(Seq::new(n).expect("从 1 起"))
}

#[tokio::test]
async fn enough_sessions_merge_after_extraction_and_land_as_events() {
    let server = Server::start(vec![
        every_turn("用户养了一只猫"),
        merged(serde_json::json!({
            "revised": [{"id": "m1", "text": "用户 2026-10-09 养了一只猫"}],
            "retired": [{"id": "m2", "why": "重复 m1"}],
            "summary": "用户养了一只猫。"
        })),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "merge_sessions = 1\n");
    let handle = session(&home, 2).await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "是只橘猫").await;
    requested(&server, 2).await;
    until_merged(&home).await;
    let text = asked(&server, 1);
    assert!(text.starts_with("Below are the memories"), "{text}");
    assert!(text.contains("Current summary: none yet."), "{text}");
    assert!(
        text.contains("Saved since the summary:\nm1 user ") && text.contains("m2 user "),
        "{text}"
    );
    assert!(
        !text.contains("Older related memories:"),
        "没有旧的：{text}"
    );
    let old = entry(&home, id(1));
    let new = book(&home, |book| {
        book.all()
            .find(|entry| entry.replaces == Some(id(1)))
            .cloned()
    })
    .expect("改了 m1");
    assert_eq!(new.text, "用户 2026-10-09 养了一只猫");
    assert_eq!(
        (&new.class, &new.sources, &new.audience),
        (&old.class, &old.sources, &old.audience),
        "类、出处、听众照旧"
    );
    assert!(matches!(&new.by, By::Module(module) if module.id.as_str() == "memory"));
    assert_eq!(old.replaced_by, Some(new.id));
    assert_eq!(entry(&home, id(2)).retired.as_deref(), Some("重复 m1"));
    assert_eq!(
        book(&home, |book| book.summary().map(str::to_string)).as_deref(),
        Some("用户养了一只猫。")
    );
    assert_eq!(
        book(&home, |book| book.merged()).map(|(_, upto)| upto),
        Seq::new(2),
        "合到新记的最后一条"
    );
    stop(&handle).await;
}

#[tokio::test]
async fn too_few_sessions_wait_and_a_merge_does_not_repeat_before_its_time() {
    let server = Server::start(vec![
        every_turn("用户养了一只猫"),
        every_turn("用户住在东京"),
        merged(serde_json::json!({"summary": "用户养猫，住在东京。"})),
        every_turn("用户周末写 Rust"),
        spare(),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "merge_sessions = 2\n");
    let first = session(&home, 2).await;
    chat(&home, &first, 1, "我养了一只猫").await;
    chat(&home, &first, 2, "橘猫").await;
    requested(&server, 1).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(server.received().len(), 1, "一个会话不够两个");
    let second = session(&home, 2).await;
    chat(&home, &second, 1, "我住在东京").await;
    chat(&home, &second, 2, "很多年了").await;
    requested(&server, 3).await;
    until_merged(&home).await;
    assert!(asked(&server, 2).starts_with("Below are the memories"));
    let third = session(&home, 2).await;
    chat(&home, &third, 1, "我周末写 Rust").await;
    chat(&home, &third, 2, "写编译器").await;
    requested(&server, 4).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(server.received().len(), 4, "合过的 24 小时内不再合");
    for handle in [&first, &second, &third] {
        stop(handle).await;
    }
}

#[tokio::test]
async fn related_old_memories_come_along_and_hidden_ones_do_not() {
    let server = Server::start(vec![
        every_turn("团子三岁了"),
        merged(serde_json::json!({})),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "merge_sessions = 1\n");
    save(&home, "用户养了一只猫，叫团子");
    // 上次合并在很久以前、合到 m1：m1 是旧的，之后的是新记的。
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let long_ago = Timestamp::parse("2026-01-01T00:00:00.000Z").expect("合写法");
    let mark = MemoryEvent::Merged(Merged {
        upto: Seq::new(1).expect("从 1 起"),
        given: 1,
        revised: 0,
        retired: 0,
        failed: false,
    });
    log.append(long_ago, alice(), None, &mark).expect("记得下");
    save(&home, "团子不爱吃鱼，已经不算了");
    let gone = log
        .book(|book| book.all().last().map(|entry| entry.id))
        .expect("有");
    let retired = MemoryEvent::Retired(Retired {
        id: gone,
        why: "不对".into(),
    });
    log.append(long_ago, alice(), None, &retired)
        .expect("记得下");
    let bob: By = serde_json::from_str(r#"{"kind":"person","account":"bob"}"#).expect("合写法");
    let hidden = Saved {
        class: "user".into(),
        text: "团子是 bob 的猫".into(),
        sources: Vec::new(),
        audience: vec![bob.clone()],
        replaces: None,
        about: None,
    };
    log.append(long_ago, bob, None, &MemoryEvent::Saved(hidden))
        .expect("记得下");
    let handle = session(&home, 2).await;
    chat(&home, &handle, 1, "团子三岁了").await;
    chat(&home, &handle, 2, "是只橘猫").await;
    requested(&server, 2).await;
    let text = asked(&server, 1);
    assert!(
        text.contains("Older related memories:\nm1 user ")
            && text.contains("用户养了一只猫，叫团子"),
        "相关的旧记忆跟着交：{text}"
    );
    assert!(!text.contains("已经不算了"), "作废的不交：{text}");
    assert!(!text.contains("bob"), "听众不合的不交：{text}");
    stop(&handle).await;
}

/// 一次请求里「新记的」那一块：到「相关的旧记忆」为止。
fn fresh_part(text: &str) -> &str {
    let start = text.find("Saved since the summary:").expect("有新记的");
    let end = text.find("Older related memories:").unwrap_or(text.len());
    &text[start..end]
}

#[tokio::test]
async fn what_does_not_fit_goes_next_and_the_merges_own_edits_are_not_new() {
    let server = Server::start(vec![
        every_turn("用户养了一只猫"),
        merged(serde_json::json!({"revised": [{"id": "m1", "text": "改过的第一条"}]})),
        merged(serde_json::json!({})),
        spare(),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "merge_sessions = 1\n");
    // 一百条长的（一条三百多字节）：一次放不下 32 KiB。
    for n in 0..100 {
        save(&home, &format!("第 {n} 条：{}", "很长的一句话".repeat(19)));
    }
    let handle = session(&home, 2).await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "橘猫").await;
    requested(&server, 3).await;
    let (first, second) = (asked(&server, 1), asked(&server, 2));
    assert!(first.len() <= 32 * 1024, "{}", first.len());
    assert!(
        fresh_part(&first).contains("m1 user ") && !first.contains("第 99 条"),
        "头一次取老的"
    );
    assert!(
        fresh_part(&second).contains("第 99 条") && !fresh_part(&second).contains("第 0 条"),
        "接着合剩下的"
    );
    assert!(
        !fresh_part(&second).contains("改过的第一条"),
        "合并自己改出来的不算新记的"
    );
    for _ in 0..200 {
        if book(&home, |book| book.merged())
            .is_some_and(|(_, upto)| upto > Seq::new(90).expect("从 1 起"))
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(server.received().len(), 3, "两次合完，没有剩下的");
    stop(&handle).await;
}

#[tokio::test]
async fn three_failures_in_a_row_skip_the_batch() {
    let server = Server::start(vec![
        every_turn("用户养了一只猫"),
        stream("not json"),
        every_turn("用户养了一只猫"),
        stream("still not json"),
        every_turn("用户养了一只猫"),
        stream("nope"),
    ])
    .await;
    let mut home = Home::new();
    organizer(
        &mut home,
        &server,
        "merge_sessions = 1\nextract_turns = 1\n",
    );
    let handle = session(&home, 3).await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    requested(&server, 2).await;
    assert_eq!(book(&home, |book| book.merged()), None, "失败的不动真相");
    chat(&home, &handle, 2, "橘猫").await;
    requested(&server, 4).await;
    chat(&home, &handle, 3, "三岁").await;
    requested(&server, 6).await;
    until_merged(&home).await;
    assert_eq!(book(&home, |book| book.summary().map(str::to_string)), None);
    let dir = home
        .root
        .account_dir(&alice_account())
        .join("modules/memory/engineer");
    assert!(
        log_lines(&dir)
            .iter()
            .any(|line| line.contains("\"ext.memory.merged\"") && line.contains("\"failed\":true")),
        "记一条失败的记号"
    );
    assert!(
        book(&home, |book| book
            .all()
            .all(
                |entry| entry.retired.is_none() && entry.replaced_by.is_none()
            )),
        "跳过的这一批什么都没改"
    );
    stop(&handle).await;
}

#[tokio::test]
async fn enough_sessions_still_wait_for_the_interval_and_revisions_keep_the_class() {
    let server = Server::start(vec![
        every_turn("用户养了一只猫"),
        merged(serde_json::json!({
            "revised": [
                {"id": "m1", "text": "回答要短，先说结论"},
                {"id": "m2", "text": "用户养了一只猫"}
            ],
            "summary": "用户养猫。"
        })),
        every_turn("用户住在东京"),
        spare(),
    ])
    .await;
    let mut home = Home::new();
    organizer(&mut home, &server, "merge_sessions = 1\n");
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let feedback = Saved {
        class: "feedback".into(),
        text: "回答要短".into(),
        sources: Vec::new(),
        audience: vec![alice()],
        replaces: None,
        about: None,
    };
    log.append(
        Timestamp::parse("2026-10-01T00:00:00.000Z").expect("合写法"),
        alice(),
        None,
        &MemoryEvent::Saved(feedback),
    )
    .expect("记得下");
    let first = session(&home, 2).await;
    chat(&home, &first, 1, "我养了一只猫").await;
    chat(&home, &first, 2, "橘猫").await;
    requested(&server, 2).await;
    until_merged(&home).await;
    let revised = book(&home, |book| {
        book.all()
            .find(|entry| entry.replaces == Some(id(1)))
            .cloned()
    })
    .expect("改了 m1");
    assert_eq!(revised.class, "feedback", "类照旧");
    assert!(
        book(&home, |book| book
            .all()
            .all(|entry| entry.replaces != Some(id(2)))),
        "和原文一样的不记"
    );
    let second = session(&home, 2).await;
    chat(&home, &second, 1, "我住在东京").await;
    chat(&home, &second, 2, "很多年了").await;
    requested(&server, 3).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(
        book(&home, |book| book.sessions_since_merge()),
        1,
        "会话够了"
    );
    assert_eq!(server.received().len(), 3, "间隔没到不合");
    for handle in [&first, &second] {
        stop(handle).await;
    }
}

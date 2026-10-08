//! 常驻的记忆摘要（施工 R-4 上，`docs/blueprint/memory.md` 第三条）：真会话、真记忆日志，数据根在临时目录。第一轮的请求里
//! 有那一块、在触发的那句前面、新的在前、一行一个写法逐字节对；第二轮不再交，第一轮那一块原样还在；清空以后又交一次；
//! 范围 `off` 的、一条都没有的不交；超过上限截在一条的边界上、说还有几条；听众不合的、作废的、改掉的不进，正文的换行换成
//! 空格；读得慢的不挡回合。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_kernel::origin::By;
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::Command;
use miyu_kernel::time::Timestamp;
use miyu_policy::memory::MemoryScope;
use miyu_recall::{MemoryEvent, MemoryId, Retired, Saved};
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_store::recall::Room;

use crate::support::*;

fn persona() -> Room {
    Room::persona(&alice_account(), "engineer")
}

/// 记下的时刻：UTC 的 10 月 7 日 20 点，会话的时区（东九区）里已经是 8 日，摘要里的日期照会话的时区写。
fn saved_at() -> Timestamp {
    Timestamp::parse("2026-10-07T20:00:00.000Z").expect("合写法")
}

/// 人在这一间里记一条（听众是 `audience`），交回编号。
fn save(
    home: &Home,
    class: &str,
    text: &str,
    audience: By,
    replaces: Option<MemoryId>,
) -> MemoryId {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let saved = Saved {
        class: class.into(),
        text: text.into(),
        sources: Vec::new(),
        audience: vec![audience.clone()],
        replaces,
        about: None,
    };
    log.append(saved_at(), audience, None, &MemoryEvent::Saved(saved))
        .expect("记得下")
        .id
}

fn retire(home: &Home, id: MemoryId) {
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let retired = MemoryEvent::Retired(Retired {
        id,
        why: "不对".into(),
    });
    log.append(saved_at(), alice(), None, &retired)
        .expect("记得下");
}

/// 一条在摘要里的样子：和 `memory_search` 的一行一个写法，日期照会话的时区（测试场地是东九区，[`saved_at`]）。
fn line(id: MemoryId, class: &str, text: &str) -> String {
    format!("{id} {class} 2026-10-08: {text}")
}

/// 说一句，等这一轮结束。
async fn chat(handle: &Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 一次请求里人那一边的字块，照先后。
fn texts(request: &Request) -> Vec<String> {
    request
        .messages
        .iter()
        .flat_map(|message| match message {
            Message::User { blocks } => blocks.clone(),
            _ => Vec::new(),
        })
        .filter_map(|block| match block {
            Block::Text(Text { text }) => Some(text),
            _ => None,
        })
        .collect()
}

/// 请求里记忆那一块（有几块交几块）。
fn summaries(request: &Request) -> Vec<String> {
    texts(request)
        .into_iter()
        .filter(|text| text.starts_with("<memories>"))
        .collect()
}

#[tokio::test]
async fn the_first_turn_has_the_block_before_the_trigger_and_later_turns_keep_it() {
    let home = Home::new();
    let cat = save(&home, "user", "用户养了一只猫", alice(), None);
    let short = save(&home, "feedback", "回答要短，先说结论", alice(), None);
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let requests = script.requests();
    let expected = format!(
        "<memories>\n{}\n{}\n</memories>\n",
        line(short, "feedback", "回答要短，先说结论"),
        line(cat, "user", "用户养了一只猫")
    );
    assert_eq!(
        summaries(&requests[0].1),
        std::slice::from_ref(&expected),
        "新的在前，一行一个写法"
    );
    let said = texts(&requests[0].1);
    let block = said
        .iter()
        .position(|text| text == &expected)
        .expect("有那一块");
    let trigger = said
        .iter()
        .position(|text| text == "在吗")
        .expect("有那句话");
    assert!(block < trigger, "在触发的那句前面：{said:?}");

    chat(&handle, "cmd-2", "再说一句").await;
    let requests = script.requests();
    assert_eq!(
        summaries(&requests[1].1),
        [expected],
        "第二轮不再交，第一轮那一块原样还在"
    );
    let injected: Vec<Vec<String>> = home
        .log(handle.id())
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextInjected(fact) if fact.kind.as_str() == "memory" => {
                Some(fact.refs.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        injected,
        [vec![short.to_string(), cat.to_string()]],
        "只交了一次，带着那几条的编号"
    );
    stop(&handle).await;
}

#[tokio::test]
async fn clearing_brings_the_block_back() {
    let home = Home::new();
    save(&home, "user", "用户养了一只猫", alice(), None);
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    within("清空", handle.command(id("cmd-2"), alice(), Command::Clear))
        .await
        .expect("会话在跑");
    chat(&handle, "cmd-3", "清空以后").await;
    let requests = script.requests();
    assert_eq!(summaries(&requests[1].1).len(), 1, "清空以后又交一次");
    stop(&handle).await;
}

#[tokio::test]
async fn nothing_for_an_off_session_or_an_empty_room() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    assert!(
        summaries(&script.requests()[0].1).is_empty(),
        "一条都没有的不交"
    );
    stop(&handle).await;

    save(&home, "user", "用户养了一只猫", alice(), None);
    let script = Script::new([Play::Says("好。")]);
    let off = Lines {
        memory: MemoryScope::Off,
        ..Lines::default()
    };
    let handle = home
        .create_full(
            &script,
            &miyu_tool::Catalog::default(),
            Opening::default(),
            off,
        )
        .await;
    chat(&handle, "cmd-1", "在吗").await;
    assert!(summaries(&script.requests()[0].1).is_empty(), "off 的不交");
    stop(&handle).await;
}

#[tokio::test]
async fn only_what_counts_and_what_they_may_hear() {
    let home = Home::new();
    let bob: By = serde_json::from_str(r#"{"kind":"person","account":"bob"}"#).expect("合写法");
    save(&home, "user", "bob 的猫叫团子", bob, None);
    let wrong = save(&home, "user", "用户用 A 卡", alice(), None);
    save(
        &home,
        "user",
        "用户用 N 卡\n型号 4090",
        alice(),
        Some(wrong),
    );
    let gone = save(&home, "user", "用户住在北京", alice(), None);
    retire(&home, gone);
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let block = summaries(&script.requests()[0].1).concat();
    assert!(
        block.contains(": 用户用 N 卡 型号 4090\n"),
        "正文的换行换成空格：{block}"
    );
    for left_out in ["团子", "A 卡", "北京"] {
        assert!(!block.contains(left_out), "{left_out} 不该进：{block}");
    }
    stop(&handle).await;
}

#[tokio::test]
async fn a_long_list_is_cut_at_a_line_and_says_how_many_more() {
    let home = Home::new();
    for n in 0..60 {
        save(
            &home,
            "episode",
            &format!("第 {n} 件事：{}", "一起做过的事".repeat(15)),
            alice(),
            None,
        );
    }
    let script = Script::new([Play::Says("好。")]);
    let handle = home.create(&script).await;
    chat(&handle, "cmd-1", "在吗").await;
    let block = summaries(&script.requests()[0].1).concat();
    assert!(block.len() <= 3000, "上限 3000 字节：{}", block.len());
    let lines: Vec<&str> = block.lines().collect();
    let shown = lines
        .iter()
        .filter(|line| line.contains(" episode "))
        .count();
    assert!(shown > 0 && shown < 60, "{shown}");
    assert_eq!(
        lines[lines.len() - 2],
        format!("…and {} more; memory_search finds them.", 60 - shown),
        "截在一条的边界上，说还有几条"
    );
    assert!(lines[1].contains("第 59 件事"), "新的在前：{}", lines[1]);
    stop(&handle).await;
}

/// 读得慢的不挡回合（`memory.md` 第三条第 4 款）：另一个线程握着记忆日志的锁，这一轮过了时限照常请求、不带那一块；放开以后
/// 下一轮再问，交了。时限是几毫秒不测（断言结果，不断言耗时）。
#[tokio::test]
async fn a_slow_read_does_not_hold_the_turn() {
    let home = Home::new();
    save(&home, "user", "用户养了一只猫", alice(), None);
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let handle = home.create(&script).await;
    let (log, _) = home.logs.open(&persona()).expect("开得了");
    let (locked, held) = std::sync::mpsc::channel();
    let (release, released) = std::sync::mpsc::channel::<()>();
    let holder = std::thread::spawn(move || {
        log.book(|_| {
            locked.send(()).expect("测试在等");
            assert!(released.recv().is_err(), "测试放开时关了这一头");
        });
    });
    held.recv().expect("握住了锁");
    chat(&handle, "cmd-1", "在吗").await;
    assert!(
        summaries(&script.requests()[0].1).is_empty(),
        "过了时限，这一轮不带"
    );
    drop(release);
    holder.join().expect("放开了");
    chat(&handle, "cmd-2", "再说一句").await;
    assert_eq!(
        summaries(&script.requests()[1].1).len(),
        1,
        "下一轮再问，交了"
    );
    stop(&handle).await;
}

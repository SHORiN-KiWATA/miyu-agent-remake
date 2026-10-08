//! 记忆的三件工具接在会话上（施工 R-3 中，`docs/blueprint/memory.md`「工具」、第二条、第九条）：真记忆日志、真回合库，执行器
//! 替身照剧本调工具，数据根在临时目录。只有本机的主会话工具面上有；她记下的出处是这一轮、`by` 是那次调用；别的会话搜得到
//! 记下的和以前的对话；撤销那一轮，那一条跟着看不见、恢复又看得见；作废的只在带 `forgotten` 时出来。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{SessionId, TurnId, VenueId};
use miyu_kernel::origin::By;
use miyu_kernel::request::Request;
use miyu_kernel::session::Command;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Handle, Lineage};
use miyu_store::recall::Room;
use miyu_tool::Catalog;

use crate::support::*;

/// 基础系统加记忆的三件，和核心里一样。
fn catalog(home: &Home) -> Catalog {
    let mut tools = miyu_basesystem::tools(home.resources.path()).expect("读得出");
    tools.extend(miyu_memory::tools(home.resources.path()).expect("读得出"));
    Catalog::new(tools).expect("合写法")
}

async fn session(home: &Home, script: &Script, lines: Lines) -> Handle {
    home.create_full(script, &catalog(home), Opening::default(), lines)
        .await
}

/// 说 `words`，等到结束了 `turns` 轮，交回日志。
async fn chat(home: &Home, handle: &Handle, turns: usize, words: &str) -> Vec<Event> {
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
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
fn last_result(log: &[Event]) -> String {
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

fn names(request: &Request) -> Vec<&str> {
    request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect()
}

fn call(name: &str, args: serde_json::Value) -> Play {
    Play::calls(&[(name, &args.to_string())])
}

/// 日志里最后一个回合。
fn last_turn(log: &[Event]) -> TurnId {
    log.iter()
        .rev()
        .find_map(|event| match event.body {
            Body::TurnStarted(_) => event.turn,
            _ => None,
        })
        .expect("有回合")
}

#[tokio::test]
async fn only_a_local_main_session_has_the_three() {
    let home = Home::new();
    let memory = ["forget", "memory_search", "remember"];
    let parent = SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d99").expect("合写法");
    for (lines, has) in [
        (Lines::default(), true),
        (
            Lines {
                lineage: Some(Lineage { parent, depth: 1 }),
                ..Lines::default()
            },
            false,
        ),
        (
            Lines {
                venue: VenueId::parse("qq:group:123456").expect("合写法"),
                ..Lines::default()
            },
            false,
        ),
    ] {
        let script = Script::new([Play::Says("好。")]);
        let handle = session(&home, &script, lines).await;
        chat(&home, &handle, 1, "看看").await;
        let requests = script.requests();
        let tools = names(&requests[0].1);
        for name in memory {
            assert_eq!(tools.contains(&name), has, "{name}：{tools:?}");
        }
    }
}

#[tokio::test]
async fn what_she_remembers_comes_from_this_turn_and_is_found_from_another_session() {
    let home = Home::new();
    let script = Script::new([
        call(
            "remember",
            serde_json::json!({"class":"user","text":"用户的显卡是 N 卡"}),
        ),
        Play::Says("记住了。"),
    ]);
    let first = session(&home, &script, Lines::default()).await;
    let log = chat(&home, &first, 1, "记住我的显卡是 N 卡").await;
    assert_eq!(last_result(&log), "Saved as m1.");
    let turn = last_turn(&log);
    let (memories, _) = home
        .logs
        .open(&Room::persona(&alice_account(), "engineer"))
        .expect("开得了");
    let entry = memories
        .book(|book| book.all().next().cloned())
        .expect("记下了一条");
    assert_eq!(entry.text, "用户的显卡是 N 卡");
    assert_eq!(entry.sources.len(), 1);
    assert_eq!(
        (&entry.sources[0].session, entry.sources[0].turn),
        (first.id(), turn)
    );
    assert!(matches!(entry.by, By::Tool(_)), "{:?}", entry.by);
    assert_eq!(entry.audience, [alice()], "听众是属主");

    let script = Script::new([
        call("memory_search", serde_json::json!({"query":"显卡"})),
        Play::Says("你用 N 卡。"),
    ]);
    let second = session(&home, &script, Lines::default()).await;
    let log = chat(&home, &second, 1, "我用什么显卡？").await;
    let found = last_result(&log);
    let lines: Vec<&str> = found.lines().collect();
    assert_eq!(lines.len(), 2, "{found}");
    assert!(
        lines[0].starts_with("m1 user ") && lines[0].ends_with(": 用户的显卡是 N 卡"),
        "{found}"
    );
    assert!(
        lines[1].contains(&format!(
            "session {}: 记住我的显卡是 N 卡 / 记住了。",
            first.id().short()
        )),
        "以前的对话：别的会话那一轮；这个会话自己的不算：{found}"
    );
}

#[tokio::test]
async fn undoing_the_turn_hides_it_and_forgetting_retires_it() {
    let home = Home::new();
    let script = Script::new([
        call(
            "remember",
            serde_json::json!({"class":"user","text":"用户养了一只猫"}),
        ),
        Play::Says("记住了。"),
    ]);
    let first = session(&home, &script, Lines::default()).await;
    let log = chat(&home, &first, 1, "我养了一只猫").await;
    let turn = last_turn(&log);
    let searching = |n: usize| {
        Script::new(
            (0..n)
                .flat_map(|_| {
                    [
                        call(
                            "memory_search",
                            serde_json::json!({"query":"猫","forgotten":true}),
                        ),
                        Play::Says("好。"),
                    ]
                })
                .collect::<Vec<_>>(),
        )
    };

    ask(&first, "undo", Command::Revert { turn: Some(turn) })
        .await
        .expect("会话在跑");
    let script = searching(1);
    let other = session(&home, &script, Lines::default()).await;
    assert_eq!(
        last_result(&chat(&home, &other, 1, "猫呢").await),
        "Nothing found.",
        "撤销了：记下的和那一轮都看不见"
    );

    ask(&first, "redo", Command::Unrevert)
        .await
        .expect("会话在跑");
    let script = Script::new([
        call("forget", serde_json::json!({"id":"m1","why":"不养了"})),
        Play::Says("忘掉了。"),
        call("memory_search", serde_json::json!({"query":"猫"})),
        Play::Says("好。"),
        call(
            "memory_search",
            serde_json::json!({"query":"猫","forgotten":true}),
        ),
        Play::Says("好。"),
    ]);
    let other = session(&home, &script, Lines::default()).await;
    assert_eq!(
        last_result(&chat(&home, &other, 1, "忘掉猫").await),
        "Retired m1."
    );
    let found = last_result(&chat(&home, &other, 2, "猫呢").await);
    assert!(!found.contains("m1"), "作废的不出来：{found}");
    let found = last_result(&chat(&home, &other, 3, "连忘了的").await);
    assert!(
        found
            .lines()
            .next()
            .is_some_and(|line| line.contains("(retired): 用户养了一只猫")),
        "{found}"
    );
}

#[tokio::test]
async fn replaced_ones_others_and_her_own_session_stay_out() {
    use miyu_recall::{MemoryEvent, Saved};
    let home = Home::new();
    // 一条听众是别人的（比如别人私聊里说的）：她在这里搜不到，也改不了。
    let (memories, _) = home
        .logs
        .open(&Room::persona(&alice_account(), "engineer"))
        .expect("开得了");
    let someone: By = serde_json::from_str(r#"{"kind":"person","account":"bob"}"#).expect("合写法");
    let private = Saved {
        class: "user".into(),
        text: "bob 的猫叫团子".into(),
        sources: Vec::new(),
        audience: vec![someone.clone()],
        replaces: None,
        about: None,
    };
    memories
        .append(now(), someone, &MemoryEvent::Saved(private))
        .expect("记得下");
    let script = Script::new([
        call(
            "remember",
            serde_json::json!({"class":"user","text":"用户的猫叫咪咪"}),
        ),
        Play::Says("记住了。"),
        call(
            "remember",
            serde_json::json!({"class":"user","text":"用户的猫叫小白","replaces":"m2"}),
        ),
        Play::Says("改好了。"),
        call(
            "memory_search",
            serde_json::json!({"query":"猫","forgotten":true}),
        ),
        Play::Says("好。"),
        call(
            "remember",
            serde_json::json!({"class":"user","text":"x","replaces":"m1"}),
        ),
        Play::Says("好。"),
        call("forget", serde_json::json!({"id":"m3","why":"说错了"})),
        Play::Says("好。"),
        call("forget", serde_json::json!({"id":"m3","why":"再忘一次"})),
        Play::Says("好。"),
        call("forget", serde_json::json!({"id":"m1","why":"忘掉 bob 的"})),
        Play::Says("好。"),
        call(
            "remember",
            serde_json::json!({"class":"user","text":"x","replaces":"m3"}),
        ),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script, Lines::default()).await;
    assert_eq!(
        last_result(&chat(&home, &handle, 1, "我的猫叫咪咪").await),
        "Saved as m2."
    );
    assert_eq!(
        last_result(&chat(&home, &handle, 2, "不对，叫小白").await),
        "Saved as m3, replacing m2."
    );
    let found = last_result(&chat(&home, &handle, 3, "猫叫什么").await);
    assert!(
        found.starts_with("m3 user ") && found.ends_with(": 用户的猫叫小白"),
        "只有改过的那一条，没有别人的、没有这个会话自己的对话：{found}"
    );
    assert_eq!(found.lines().count(), 1, "{found}");
    assert_eq!(
        last_result(&chat(&home, &handle, 4, "改 bob 的").await),
        "There is no memory m1.",
        "听众不合的当没有"
    );
    assert_eq!(
        last_result(&chat(&home, &handle, 5, "忘掉").await),
        "Retired m3."
    );
    assert_eq!(
        last_result(&chat(&home, &handle, 6, "再忘").await),
        "m3 was already replaced or retired."
    );
    assert_eq!(
        last_result(&chat(&home, &handle, 7, "忘掉 bob 的").await),
        "There is no memory m1.",
        "听众不合的作废不了"
    );
    assert_eq!(
        last_result(&chat(&home, &handle, 8, "再改").await),
        "m3 was already replaced or retired."
    );
}

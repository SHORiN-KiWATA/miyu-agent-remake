//! `miyu memory`（施工 R-3 再补，`docs/blueprint/cli/memory.md`）：在进程里起一个核心，每个子命令走一遍：印什么、退出码；
//! `--persona`、`-s` 找哪一间；`clear session` 不写会话的照上一次 `miyu ask`；没设默认人格、核心拒的照原话说。

use std::sync::Arc;

use clap::Parser;
use serde_json::Value;

use crate::support::{Asked, Home, Tape, plan, within};
use miyu_cli::language::Language;
use miyu_cli::{Memory, MemoryPlan, memory_on};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_session::testkit::{Play, Script};

/// 照主程序的样子读参数：`miyu memory …`。
#[derive(Parser)]
struct Line {
    #[command(subcommand)]
    command: Top,
}

#[derive(clap::Subcommand)]
enum Top {
    Memory(Memory),
}

fn parsed(words: &[&str]) -> Result<Memory, clap::Error> {
    let line = std::iter::once("miyu").chain(words.iter().copied());
    Line::try_parse_from(line).map(|line| match line.command {
        Top::Memory(memory) => memory,
    })
}

/// 系统配置里默认人格是软件工程师的核心。
fn home() -> Home {
    Home::configured(
        Arc::new(Script::new([Play::Says("好。")])),
        "[persona]\ndefault = \"engineer\"\n",
    )
}

/// 在真的套接字上连上核心，照 `words`（`memory` 后面的）跑一次；界面是中文，时区是零时区。
async fn run(home: &Home, words: &[&str]) -> Asked {
    let mut line = vec!["memory"];
    line.extend_from_slice(words);
    let plan = MemoryPlan {
        args: parsed(&line).expect("参数读得懂"),
        language: Language::Chinese,
        offset: UtcOffset::UTC,
    };
    let (connection, token) = miyu_ipc::connect(&home.root).await.expect("连得上");
    let tape = Tape::default();
    let (mut out, mut err) = (tape.pen(false), tape.pen(true));
    let code = within(
        "跑完",
        memory_on(connection, &token, &plan, &mut out, &mut err),
    )
    .await;
    Asked {
        code,
        out: tape.text(|err| !err),
        err: tape.text(|err| err),
        screen: tape.text(|_| true),
    }
}

/// 照 `--format json` 读回的那一串，算出 `list` 该印的几行：日期照零时区。
async fn expected(home: &Home, words: &[&str]) -> String {
    let mut line = words.to_vec();
    line.extend(["--format", "json"]);
    let json: Value = serde_json::from_str(&run(home, &line).await.out).expect("是 JSON");
    let memories = json.as_array().expect("一串");
    let width = memories
        .iter()
        .map(|memory| memory["id"].as_str().unwrap_or_default().len())
        .max()
        .unwrap_or(0);
    memories
        .iter()
        .map(|memory| {
            let id = memory["id"].as_str().unwrap_or_default();
            let at = Timestamp::parse(memory["at"].as_str().unwrap_or_default()).expect("合写法");
            let mut row = format!(
                "{id:width$}  {}  {}",
                at.local_date(UtcOffset::UTC),
                memory["text"].as_str().unwrap_or_default()
            );
            match memory["retired"].as_str() {
                Some("") => row.push_str("（已作废）"),
                Some(why) => row.push_str(&format!("（已作废：{why}）")),
                None => {}
            }
            row + "\n"
        })
        .collect()
}

#[tokio::test]
async fn list_add_search_edit_and_forget() {
    let home = home();
    let empty = run(&home, &[]).await;
    assert_eq!(
        (empty.code, empty.out.as_str()),
        (0, "还没有记忆。\n"),
        "不写子命令就是 list"
    );
    let added = run(&home, &["add", "用户养了一只猫"]).await;
    assert_eq!(
        (added.code, added.out.as_str()),
        (0, "记下了：m1\n"),
        "{}",
        added.err
    );
    let added = run(
        &home,
        &["add", "--class", "feedback", "回答要短，", "先说结论"],
    )
    .await;
    assert_eq!(added.out, "记下了：m2\n");

    let listed = run(&home, &["list"]).await;
    assert_eq!(listed.code, 0);
    assert_eq!(listed.out, expected(&home, &["list"]).await);
    let rows: Vec<&str> = listed.out.lines().collect();
    assert_eq!(rows.len(), 2, "{}", listed.out);
    assert!(
        rows[0].starts_with("m2  ") && rows[0].ends_with("  回答要短， 先说结论"),
        "新的在前，几个词用空格连起来：{rows:?}"
    );
    let users = run(&home, &["list", "--class", "user"]).await;
    assert_eq!(users.out.lines().count(), 1, "{}", users.out);
    assert!(users.out.contains("用户养了一只猫"));
    let json: Value = serde_json::from_str(&run(&home, &["list", "--format", "json"]).await.out)
        .expect("是 JSON");
    assert_eq!(json[0]["by"], "person", "json 是核心回的原样");

    let found = run(&home, &["search", "猫"]).await;
    assert!(
        found.out.starts_with("m1  ") && found.out.ends_with("  用户养了一只猫\n"),
        "{}",
        found.out
    );
    let none = run(&home, &["search", "狗"]).await;
    assert_eq!((none.code, none.out.as_str()), (0, "没找到。\n"));

    let edited = run(&home, &["edit", "m1", "用户养了两只猫"]).await;
    assert_eq!(
        (edited.code, edited.out.as_str()),
        (0, "改好了：m3\n"),
        "{}",
        edited.err
    );
    let forgot = run(&home, &["forget", "m3", "--why", "试一下"]).await;
    assert_eq!(
        (forgot.code, forgot.screen.as_str()),
        (0, ""),
        "忘掉什么都不印"
    );
    let listed = run(&home, &["list"]).await;
    assert_eq!(
        listed.out.lines().count(),
        1,
        "改掉的、忘掉的不出来：{}",
        listed.out
    );
    let all = run(&home, &["list", "--forgotten"]).await;
    assert!(
        all.out.contains("用户养了两只猫（已作废：试一下）"),
        "{}",
        all.out
    );
    assert_eq!(all.out, expected(&home, &["list", "--forgotten"]).await);

    let again = run(&home, &["edit", "m1", "再改一次"]).await;
    assert_eq!(again.code, 1);
    assert_eq!(
        again.err, "这一条已经改掉、作废或者清掉了。\n",
        "照核心的原话"
    );
    let long = "长".repeat(121);
    let too_long = run(&home, &["add", &long]).await;
    assert_eq!(too_long.code, 1);
    assert_eq!(
        too_long.err, "太长了：这一条 121 个字，一条最多 120 个字。\n",
        "照 data 说清楚，不说「在 data 里」"
    );
    let unknown = run(&home, &["forget", "m99"]).await;
    assert_eq!(
        (unknown.code, unknown.err.as_str()),
        (1, "没有这一条记忆。\n")
    );
}

#[tokio::test]
async fn clearing_one_session_or_everything() {
    let home = home();
    let none = run(&home, &["clear", "session"]).await;
    assert_eq!(none.code, 1);
    assert_eq!(
        none.err, "还没有 miyu ask 开过的会话\n",
        "不写会话、又没有 miyu ask 开过的"
    );

    assert_eq!(home.ask(&plan("在吗")).await.code, 0);
    run(&home, &["add", "用户住在上海"]).await;
    let cleared = run(&home, &["clear", "session"]).await;
    assert_eq!(
        (cleared.code, cleared.out.as_str()),
        (0, "清掉了 0 条。\n"),
        "{}",
        cleared.err
    );
    let mine = run(&home, &["clear", "me"]).await;
    assert_eq!(mine.out, "清掉了 1 条。\n");
    assert_eq!(
        run(&home, &["list", "--forgotten"]).await.out,
        "还没有记忆。\n"
    );

    // 会话那一间：-s 记进去，清掉这个会话的就是整间。
    let own = home
        .create_with(serde_json::json!({"cwd": "/work", "memory": "session"}))
        .await;
    run(&home, &["add", "-s", &own, "一"]).await;
    run(&home, &["add", "二", "--session", &own]).await;
    assert_eq!(
        run(&home, &["list", "-s", &own]).await.out.lines().count(),
        2
    );
    assert_eq!(
        run(&home, &["list"]).await.out,
        "还没有记忆。\n",
        "人格那一间没有它们"
    );
    let cleared = run(&home, &["clear", "session", &own]).await;
    assert_eq!(cleared.out, "清掉了 2 条。\n");
}

#[tokio::test]
async fn which_room_and_wrong_arguments() {
    let home = Home::new(Arc::new(Script::new([])));
    let no_persona = run(&home, &["list"]).await;
    assert_eq!(no_persona.code, 1, "没设默认人格：照核心的原话");
    assert!(no_persona.err.contains("没有人格"), "{}", no_persona.err);
    let named = run(&home, &["add", "--persona", "engineer", "用户养猫"]).await;
    assert_eq!(
        (named.code, named.out.as_str()),
        (0, "记下了：m1\n"),
        "{}",
        named.err
    );
    assert_eq!(
        run(&home, &["list", "--persona", "engineer"])
            .await
            .out
            .lines()
            .count(),
        1
    );

    for words in [
        vec!["memory", "list", "--persona", "engineer", "-s", "x"],
        vec!["memory", "add", "--class", "nope", "x"],
        vec!["memory", "add"],
        vec!["memory", "edit", "m1"],
        vec!["memory", "clear"],
        vec!["memory", "drop"],
    ] {
        assert!(parsed(&words).is_err(), "{words:?} 该是参数不对");
    }
}

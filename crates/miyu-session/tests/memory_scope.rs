//! 记忆的范围（施工 R-3 下，`docs/blueprint/memory.md`「范围」）：真会话、真记忆日志、真回合库，数据根在临时目录。`off` 的
//! 工具面上没有三件、说过的不进回合库，载入以后照旧；`session` 的记在会话自己的目录里，别的会话搜不到，载入以后接着记在
//! 那里；子会话的快照写 `off`。

mod support;

use miyu_kernel::event::Body;
use miyu_kernel::id::SessionId;
use miyu_policy::Snapshot;
use miyu_policy::memory::MemoryScope;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Handle, Lineage};
use miyu_store::blob::Blobs;
use miyu_store::recall::Room;
use miyu_tool::Catalog;

use support::*;

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

fn scoped(memory: MemoryScope) -> Lines {
    Lines {
        memory,
        ..Lines::default()
    }
}

/// 说一句，等这一轮结束。
async fn chat(handle: &Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 那一间的回合库里搜 `words`，交回键。
fn found(home: &Home, room: &Room, words: &str) -> Vec<String> {
    let (index, _) = home.recall.turns(room);
    index
        .search(words, 10)
        .expect("搜得了")
        .into_iter()
        .map(|hit| hit.key)
        .collect()
}

fn persona() -> Room {
    Room::persona(&alice_account(), "engineer")
}

/// 会话 `session` 的策略快照。
fn snapshot(home: &Home, session: &SessionId) -> Snapshot {
    let log = home.log(session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话：{:?}", log[0]);
    };
    let bytes = Blobs::new(home.root.blobs(&alice_account()))
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂")
}

/// 一次请求的工具面里有没有记忆的三件：要么都有，要么都没有。
fn has_memory_tools(script: &Script) -> bool {
    let requests = script.requests();
    let names: Vec<&str> = requests[0]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    let has: Vec<bool> = ["forget", "memory_search", "remember"]
        .iter()
        .map(|name| names.contains(name))
        .collect();
    assert!(has.iter().all(|one| *one == has[0]), "{names:?}");
    has[0]
}

#[tokio::test]
async fn an_off_session_has_no_tools_and_keeps_no_turns_even_after_loading() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let handle = session(&home, &script, scoped(MemoryScope::Off)).await;
    let id = handle.id().clone();
    chat(&handle, "cmd-1", "我的显卡是 N 卡").await;
    assert!(!has_memory_tools(&script), "off 的工具面上没有三件");
    assert_eq!(snapshot(&home, &id).memory_scope(), MemoryScope::Off);
    stop(&handle).await;

    let script = Script::new([Play::Says("好。")]);
    let handle = home.load_with(&id, &script, &catalog(&home)).await;
    chat(&handle, "cmd-2", "我的内存是 32G").await;
    stop(&handle).await;
    for words in ["显卡", "内存"] {
        assert!(found(&home, &persona(), words).is_empty(), "{words}");
        assert!(
            found(&home, &Room::session(&alice_account(), &id), words).is_empty(),
            "{words}"
        );
    }
}

#[tokio::test]
async fn a_session_scope_lives_in_its_own_directory() {
    let home = Home::new();
    let remember = Play::calls(&[("remember", r#"{"class":"user","text":"用户养了一只猫"}"#)]);
    let script = Script::new([remember, Play::Says("记住了。")]);
    let first = session(&home, &script, scoped(MemoryScope::Session)).await;
    let id = first.id().clone();
    chat(&first, "cmd-1", "我养了一只猫").await;
    assert!(has_memory_tools(&script), "session 的有三件");
    assert_eq!(snapshot(&home, &id).memory_scope(), MemoryScope::Session);
    let room = Room::session(&alice_account(), &id);
    assert_eq!(found(&home, &room, "养了").len(), 1, "这一轮进会话那一间");
    assert!(
        found(&home, &persona(), "养了").is_empty(),
        "不进人格那一间"
    );
    let (log, _) = home.logs.open(&room).expect("开得了");
    assert_eq!(log.book(|book| book.all().count()), 1, "记在会话那一间");
    let (shared, _) = home.logs.open(&persona()).expect("开得了");
    assert_eq!(shared.book(|book| book.all().count()), 0);
    let dir = home.root.session_dir(&alice_account(), &id).join("memory");
    assert!(
        dir.join("log").is_dir() && dir.join("turns.db").is_file(),
        "{dir:?}"
    );

    // 别的会话（跟着人格的）搜不到。
    let search = || Play::calls(&[("memory_search", r#"{"query":"猫"}"#)]);
    let script = Script::new([search(), Play::Says("不知道。")]);
    let other = session(&home, &script, Lines::default()).await;
    chat(&other, "cmd-1", "我养了什么？").await;
    let said = home.log(other.id());
    assert!(
        said.iter()
            .any(|event| matches!(&event.body, Body::ToolResult(result)
            if format!("{:?}", result.blocks).contains("Nothing found."))),
        "{said:?}"
    );

    // 载入以后接着记在会话那一间。
    stop(&first).await;
    let script = Script::new([Play::Says("好。")]);
    let first = home.load_with(&id, &script, &catalog(&home)).await;
    chat(&first, "cmd-2", "它叫年糕").await;
    assert_eq!(found(&home, &room, "年糕").len(), 1);
    assert!(found(&home, &persona(), "年糕").is_empty());
    stop(&first).await;
}

#[tokio::test]
async fn a_child_is_off_whatever_it_was_given() {
    let home = Home::new();
    let parent = home.create(&Script::new([])).await;
    let script = Script::new([Play::Says("好。")]);
    let lines = Lines {
        lineage: Some(Lineage {
            parent: parent.id().clone(),
            depth: 1,
        }),
        memory: MemoryScope::Persona,
        ..Lines::default()
    };
    let child = session(&home, &script, lines).await;
    assert_eq!(snapshot(&home, child.id()).memory_scope(), MemoryScope::Off);
    let script = Script::new([Play::Says("好。")]);
    let main = session(&home, &script, Lines::default()).await;
    assert_eq!(
        snapshot(&home, main.id()).memory_scope(),
        MemoryScope::Persona,
        "主会话照交的写"
    );
    assert!(snapshot(&home, main.id()).memory.is_some(), "新造的写明");
}

/// 记忆放在记忆账号的那一间，听众照会话的属主（施工 R-3 下：两样分开）。
#[tokio::test]
async fn the_room_follows_the_memory_account_and_the_hearer_is_the_owner() {
    let home = Home::new();
    let admin = miyu_kernel::id::AccountId::parse("admin").expect("合写法");
    let remember = Play::calls(&[("remember", r#"{"class":"user","text":"用户爱喝茶"}"#)]);
    let script = Script::new([remember, Play::Says("记住了。")]);
    let lines = Lines {
        memory_account: admin.clone(),
        ..Lines::default()
    };
    let handle = session(&home, &script, lines).await;
    chat(&handle, "cmd-1", "我爱喝茶").await;
    let (log, _) = home
        .logs
        .open(&Room::persona(&admin, "engineer"))
        .expect("开得了");
    let entry = log
        .book(|book| book.all().next().cloned())
        .expect("记在记忆账号那一间");
    assert_eq!(entry.audience, [alice()], "听众是属主");
    let (own, _) = home.logs.open(&persona()).expect("开得了");
    assert_eq!(own.book(|book| book.all().count()), 0, "属主那一间没有");
    assert_eq!(
        found(&home, &Room::persona(&admin, "engineer"), "喝茶").len(),
        1
    );
}

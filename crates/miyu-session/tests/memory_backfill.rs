//! 补齐旧会话（施工 R-2 下，`docs/blueprint/memory.md`「怎么走」第一条第 9 款）：真会话、真回合库，数据根在临时目录。以前的
//! 版本造的会话回合库里一条都没有，一份新的登记第一次开人格那一间（新建），后台把这个账号跟着人格的旧会话补回去；子会话、范围 `off` 的、只在会话里
//! 的、场所会话（施工 R-2 再补）不补进来；读不了的会话跳过，别的照补。

use std::time::{Duration, Instant};

use miyu_policy::memory::MemoryScope;
use miyu_session::testkit::{Play, Script};
use miyu_session::{Handle, Lineage, Memory};
use miyu_store::recall::Room;

use crate::support::*;

/// 说一句，等这一轮结束，停下。
async fn said(home: &Home, mut lines: Lines, words: &str) -> Handle {
    lines.indexed = false;
    let script = Script::new([Play::Says("好。")]);
    let handle = home
        .create_full(
            &script,
            &miyu_tool::Catalog::default(),
            Opening::default(),
            lines,
        )
        .await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&handle).await;
    handle
}

fn persona() -> Room {
    Room::persona(&alice_account(), "engineer")
}

/// 那一间的回合库里搜 `words`，交回键。
fn found(memory: &Memory, room: &Room, words: &str) -> Vec<String> {
    let (index, _) = memory.turns.turns(room);
    index
        .search(words, 10)
        .expect("搜得了")
        .into_iter()
        .map(|hit| hit.key)
        .collect()
}

#[tokio::test]
async fn a_new_persona_index_is_filled_from_the_old_sessions() {
    let home = Home::new();
    let cat = said(&home, Lines::default(), "我养了一只猫").await;
    let hike = said(&home, Lines::default(), "周末去爬山").await;
    let parent = cat.id().clone();
    let child = Lines {
        lineage: Some(Lineage { parent, depth: 1 }),
        ..Lines::default()
    };
    said(&home, child, "子代理查到了樱花").await;
    let off = Lines {
        memory: MemoryScope::Off,
        ..Lines::default()
    };
    said(&home, off, "我的显卡是 N 卡").await;
    let own = Lines {
        memory: MemoryScope::Session,
        ..Lines::default()
    };
    said(&home, own, "我在学 Rust").await;
    // 场所会话（群里）的先不进回合库（施工 R-2 再补）：回合库的条目还没有听众。
    let venue = Lines {
        venue: miyu_kernel::id::VenueId::parse("qq:group:5550").expect("合写法"),
        ..Lines::default()
    };
    said(&home, venue, "群里说的团子").await;
    // 一个读不了的会话：目录在、日志是坏的。跳过它，别的照补（最后造的：会话照新的在前列，它排在最前，读到它不该停下）。
    let broken = home
        .root
        .session_dir(&alice_account(), &miyu_session::new_id(now()));
    std::fs::create_dir_all(&broken).expect("建得了");
    std::fs::write(broken.join("000000000001.jsonl"), "不是事件\n也不是\n").expect("写得进");

    assert!(
        !home
            .root
            .index(&alice_account())
            .join("recall/turns-engineer.db")
            .exists(),
        "以前的版本造的会话：回合库还没有"
    );
    let memory = Memory::new(&home.root, None);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let cats = found(&memory, &persona(), "养了");
        let hikes = found(&memory, &persona(), "爬山");
        if cats.len() == 1 && hikes.len() == 1 {
            assert!(cats[0].starts_with(&format!("{}/", cat.id())), "{cats:?}");
            assert!(
                hikes[0].starts_with(&format!("{}/", hike.id())),
                "{hikes:?}"
            );
            break;
        }
        assert!(Instant::now() < deadline, "等不到补齐：{cats:?} {hikes:?}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    for words in ["樱花", "显卡", "Rust", "团子"] {
        assert!(
            found(&memory, &persona(), words).is_empty(),
            "{words} 不该补进人格那一间"
        );
    }
}

/// 回合库坏了、重建过的（`Opened::Rebuilt`）也补。
#[tokio::test]
async fn a_rebuilt_persona_index_is_filled_too() {
    let home = Home::new();
    let cat = said(&home, Lines::default(), "我养了一只猫").await;
    let path = home
        .root
        .index(&alice_account())
        .join("recall/turns-engineer.db");
    std::fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了");
    std::fs::write(&path, "这不是一个库").expect("写得进");
    let memory = Memory::new(&home.root, None);
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let cats = found(&memory, &persona(), "养了");
        if cats.len() == 1 {
            assert!(cats[0].starts_with(&format!("{}/", cat.id())), "{cats:?}");
            break;
        }
        assert!(Instant::now() < deadline, "等不到补齐：{cats:?}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

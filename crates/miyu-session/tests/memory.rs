//! 回合索引（施工 R-2 上，`docs/blueprint/memory.md`「怎么走」第一条）：真会话、执行器替身，数据根在临时目录。说过的每一轮
//! 进这个人格的回合库；撤销的拿掉、恢复的放回；核心重启载入以后照旧，落下的补上；子会话的不进。

mod support;

use miyu_kernel::id::{SessionId, TurnId};
use miyu_kernel::session::Command;
use miyu_recall::Source;
use miyu_session::Handle;
use miyu_session::Lineage;
use miyu_session::testkit::{Play, Script};
use miyu_store::recall::Room;

use support::*;

/// 软件工程师的回合库里搜 `words`，交回键。
fn found(home: &Home, words: &str) -> Vec<String> {
    let (index, _) = home
        .recall
        .turns(&Room::persona(&alice_account(), "engineer"));
    index
        .search(words, 10)
        .expect("搜得了")
        .into_iter()
        .map(|hit| hit.key)
        .collect()
}

/// 说一句，等这一轮结束。
async fn chat(handle: &Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 日志里最后一个回合的编号。
fn last_turn(home: &Home, session: &SessionId) -> TurnId {
    home.log(session)
        .iter()
        .rev()
        .find_map(|event| match event.body {
            miyu_kernel::event::Body::TurnStarted(_) => event.turn,
            _ => None,
        })
        .expect("有回合")
}

fn key(session: &SessionId, turn: TurnId) -> String {
    format!("{session}/{}", turn.started().get())
}

#[tokio::test]
async fn each_turn_goes_in_and_undo_and_restore_follow() {
    let home = Home::new();
    let script = Script::new([Play::Says("好，周末去看樱花。"), Play::Says("爬山也行。")]);
    let handle = home.create(&script).await;
    let id = handle.id().clone();
    chat(&handle, "cmd-1", "周末想出去玩").await;
    let first = last_turn(&home, &id);
    chat(&handle, "cmd-2", "那去爬山呢").await;
    let second = last_turn(&home, &id);
    assert_eq!(found(&home, "樱花"), [key(&id, first)]);
    assert_eq!(found(&home, "爬山"), [key(&id, second)]);

    let alive = |turn: TurnId| {
        let source = Source {
            session: id.clone(),
            turn,
        };
        home.recall
            .alive(&Room::persona(&alice_account(), "engineer"), &source)
            .expect("读得了")
    };
    assert!(alive(first) && alive(second));

    let undone = ask(&handle, "cmd-3", Command::Revert { turn: Some(second) }).await;
    assert!(undone.is_ok(), "{undone:?}");
    assert!(found(&home, "爬山").is_empty(), "撤销的拿掉");
    assert_eq!(found(&home, "樱花"), [key(&id, first)], "前一轮还在");
    assert!(alive(first) && !alive(second), "撤销的那一轮埋了墓碑");

    let restored = ask(&handle, "cmd-4", Command::Unrevert).await;
    assert!(restored.is_ok(), "{restored:?}");
    assert_eq!(
        found(&home, "爬山"),
        [key(&id, second)],
        "恢复的读回日志放回"
    );
    assert!(alive(second), "恢复的揭掉墓碑");
}

#[tokio::test]
async fn loading_catches_up_what_was_missed() {
    let home = Home::new();
    let script = Script::new([Play::Says("记住了，你用 N 卡。")]);
    let handle = home.create(&script).await;
    let id = handle.id().clone();
    chat(&handle, "cmd-1", "我的显卡是 N 卡").await;
    let turn = last_turn(&home, &id);
    stop(&handle).await;

    // 库里落下了这个会话（更新失败、崩在更新之前）：载入时照整份事件补回来。
    let (index, _) = home
        .recall
        .turns(&Room::persona(&alice_account(), "engineer"));
    index.forget(&id.to_string()).expect("拿得掉");
    assert!(found(&home, "显卡").is_empty());
    let handle = home.load(&id, &Script::new([])).await;
    assert_eq!(found(&home, "显卡"), [key(&id, turn)]);
    stop(&handle).await;

    // 照到了的不重写：再载入一次，还是那一条。
    let handle = home.load(&id, &Script::new([])).await;
    assert_eq!(found(&home, "显卡"), [key(&id, turn)]);
    stop(&handle).await;
}

#[tokio::test]
async fn a_child_session_is_not_indexed() {
    let home = Home::new();
    let parent = home.create(&Script::new([])).await;
    let script = Script::new([Play::Says("子代理查到了：樱花开了。")]);
    let lines = Lines {
        lineage: Some(Lineage {
            parent: parent.id().clone(),
            depth: 1,
        }),
        ..Lines::default()
    };
    let child = home
        .create_full(&script, &Default::default(), Opening::default(), lines)
        .await;
    chat(&child, "cmd-1", "去查查樱花开了没有").await;
    assert!(found(&home, "樱花").is_empty());
}

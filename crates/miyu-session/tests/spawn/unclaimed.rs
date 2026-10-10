//! 派到一半被打断（施工 7-5 补，`agents.md` 第一条第 9 条）：子会话造好了、交代还没送进去，打断父会话，派子代理那次调用被
//! 掐掉，造好的子会话停掉，不留没人管的子代理（CI run 1622 上 `preset_background` 偶发红查出来的）。

use std::time::Duration;

use miyu_kernel::session::Queued;

use super::*;

/// 等到 `done` 成立，最多六十秒。
async fn until(what: &str, done: impl Fn() -> bool) {
    let waited = tokio::time::timeout(Duration::from_secs(60), async {
        while !done() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "六十秒内没等到{what}");
}

#[tokio::test]
async fn interrupted_while_handing_over_the_task_the_child_is_stopped() {
    let home = Home::new();
    let table = Arc::new(Table {
        hold: Some(Arc::new(tokio::sync::Notify::new())),
        ..Table::default()
    });
    let script = Script::new([calls(&[subagent("查", "Look around.")]), Play::Says("嗯。")]);
    let handle = parent(&home, &script, &table).await;
    ask(&handle, "cmd-1", say("去查一下"))
        .await
        .expect("会话在跑");
    until("子会话造好、交代送进去卡住", || {
        table.sent().len() == 1
    })
    .await;
    assert!(table.stopped().is_empty(), "还没打断");
    ask(
        &handle,
        "cmd-2",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await
    .expect("打断收下了");
    until("停掉造好的子会话", || {
        table.stopped().contains(&child_id(1))
    })
    .await;
}

/// 正常派出去的不停：交回了，守着的就不管了。
#[tokio::test]
async fn a_child_handed_its_task_keeps_running() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        calls(&[subagent("查", "Look around.")]),
        Play::Says("派出去了。"),
    ]);
    let handle = parent(&home, &script, &table).await;
    one_turn(&home, &handle, 1).await;
    assert_eq!(table.made().len(), 1);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(table.stopped().is_empty(), "{:?}", table.stopped());
}

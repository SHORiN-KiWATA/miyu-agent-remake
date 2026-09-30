//! 删会话之前停下（施工 3-8 三补，`docs/blueprint/session/actor.md` 第 9 条）：`delete` 先问内核，有回合在进行的说删不了、
//! 会话照常；删得了的，后台命令整组杀掉、不记回报，回了以后会话就停了。`discard` 不问：跑到一半的也停，后台命令一样杀掉、
//! 不记，那一轮不收尾。假的后台命令（`miyu_tool::testkit::Held`），三个平台一样。

mod support;

use std::sync::Arc;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::session::{Command, Outcome, Queued, Reason};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake, Held};
use miyu_tool::{Catalog, Tool};

use support::*;

/// 只有一件假工具 `start`：把 `held` 交给任务端口。
fn starting(held: &Arc<Held>) -> Catalog {
    let tool = Fake::new("start", Access::Read, Act::Background(Arc::clone(held)));
    Catalog::new([tool as Arc<dyn Tool>]).expect("合写法")
}

/// 日志里有没有后台命令结束的记录。
fn reported(log: &[Event]) -> bool {
    log.iter()
        .any(|event| matches!(event.body, Body::JobReported(_)))
}

#[tokio::test]
async fn an_idle_session_kills_its_commands_unrecorded_and_stops() {
    let home = Home::new();
    let held = Held::new(&[]);
    let script = Script::new([Play::calls(&[("start", "{}")]), Play::Says("放出去了。")]);
    let handle = home.create_with(&script, &starting(&held)).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("放一个"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let before = home.log(handle.id());
    let deleted = within("删之前停下", handle.delete()).await;
    assert_eq!(deleted, Ok(Ok(())));
    assert_eq!(held.killed(), 1, "回之前整组杀掉");
    assert!(!home.jobs.running(), "任务表里没有它的了");
    let after = home.log(handle.id());
    assert_eq!(after, before, "什么都不再写");
    assert!(!reported(&after), "不记回报");
    // 会话停了：连不用落盘的命令也收不到（还在跑的话，打断会被拒成没有回合在进行）。
    let interrupted = ask(
        &handle,
        "cmd-2",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await;
    assert!(interrupted.is_err(), "会话停了：{interrupted:?}");
}

#[tokio::test]
async fn a_running_turn_says_no_and_the_session_carries_on() {
    let home = Home::new();
    let script = Script::new([Play::Holds]);
    let handle = home.create(&script).await;
    ask(&handle, "cmd-1", say("在吗")).await.expect("会话在跑");
    let refused = within("删之前停下", handle.delete()).await;
    assert_eq!(refused, Ok(Err(Reason::TurnRunning)));
    let interrupted = ask(
        &handle,
        "cmd-2",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await;
    assert!(
        matches!(interrupted, Ok(Outcome::Accepted { .. })),
        "会话照常：{interrupted:?}"
    );
    let deleted = within("删之前停下", handle.delete()).await;
    assert_eq!(deleted, Ok(Ok(())), "打断以后删得了");
}

#[tokio::test]
async fn discarding_stops_a_busy_session_and_kills_its_commands_unrecorded() {
    let home = Home::new();
    let held = Held::new(&[]);
    let script = Script::new([Play::calls(&[("start", "{}")]), Play::Holds]);
    let handle = home.create_with(&script, &starting(&held)).await;
    ask(&handle, "cmd-1", say("放一个"))
        .await
        .expect("会话在跑");
    within("停在第二次请求上", async {
        while script.requests().len() < 2 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(home.jobs.running(), "后台命令在跑");
    within("停下", handle.discard())
        .await
        .expect("会话在跑，停得下");
    assert_eq!(held.killed(), 1, "回之前整组杀掉");
    assert!(!home.jobs.running());
    let log = home.log(handle.id());
    assert!(!reported(&log), "不记回报");
    assert!(
        !log.iter()
            .any(|event| matches!(event.body, Body::TurnEnded(_))),
        "跑到一半的那一轮不收尾"
    );
    assert!(
        ask(&handle, "cmd-2", say("还在吗")).await.is_err(),
        "会话停了"
    );
}

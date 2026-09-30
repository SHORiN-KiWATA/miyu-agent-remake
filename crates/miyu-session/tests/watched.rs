//! 有没有头看着（施工 7-9，`docs/blueprint/session/actor.md`「收件箱」第 4 条）：会话 actor 在有头订阅、全都退订时交内核
//! `Watched`。一次性的会话有头订阅着，后台命令结束叫醒她；几个头走了一个，照样叫醒；订阅都放下了，只记下。造会话以后
//! 没人订阅过的，当没人看着。假的后台命令（`miyu_tool::testkit::Held`）什么时候结束由测试定。

mod support;

use std::sync::Arc;

use miyu_kernel::event::{Body, Event};
use miyu_kernel::session::{Command, Queued};
use miyu_kernel::tool::Access;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake, Held};
use miyu_tool::{Catalog, Exit, Tool};

use support::*;

/// 两件假工具 `start`、`again`：各把一条假的后台命令交给任务端口。
fn starting(first: &Arc<Held>, second: &Arc<Held>) -> Catalog {
    let tools = [
        Fake::new("start", Access::Read, Act::Background(Arc::clone(first))) as Arc<dyn Tool>,
        Fake::new("again", Access::Read, Act::Background(Arc::clone(second))) as Arc<dyn Tool>,
    ];
    Catalog::new(tools).expect("合写法")
}

/// 造一个一次性的会话：第一轮放一条后台命令 `start`；它结束叫醒的那一轮再放一条 `again`。
async fn oneshot(home: &Home, first: &Arc<Held>, second: &Arc<Held>) -> Handle {
    let script = Script::new([
        Play::calls(&[("start", "{}")]),
        Play::Says("放出去了。"),
        Play::calls(&[("again", "{}")]),
        Play::Says("又放了一个。"),
    ]);
    let lines = Lines {
        oneshot: true,
        ..Lines::default()
    };
    home.create_full(&script, &starting(first, second), Opening::default(), lines)
        .await
}

/// 日志里开了几轮、记了几条后台命令结束。
fn counted(log: &[Event]) -> (usize, usize) {
    let count = |kind: fn(&Body) -> bool| log.iter().filter(|event| kind(&event.body)).count();
    (
        count(|body| matches!(body, Body::TurnStarted(_))),
        count(|body| matches!(body, Body::JobReported(_))),
    )
}

/// 等收件箱里前面的都办完了：闲着时打断会被当场拒掉，回应到了，排在它前面的放下订阅也送进内核了。
async fn settled(handle: &Handle, id: &str) {
    let refused = ask(
        handle,
        id,
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await;
    assert!(refused.is_ok(), "会话在跑：{refused:?}");
}

#[tokio::test]
async fn heads_watching_let_the_report_wake_her_until_the_last_one_leaves() {
    let home = Home::new();
    let (first, second) = (Held::new(&[]), Held::new(&[]));
    let handle = oneshot(&home, &first, &second).await;
    let mut staying = watch(&handle).await;
    let leaving = watch(&handle).await;
    ask(&handle, "cmd-1", say("放一个"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut staying).await;
    // 走了一个头，还有一个看着：结束了叫醒她。
    drop(leaving);
    settled(&handle, "cmd-2").await;
    first.end(Exit::Code(0));
    until_turn_ends(&mut staying).await;
    assert_eq!(counted(&home.log(handle.id())), (2, 1), "叫醒她开了一轮");
    // 最后一个头也走了：只记下，不开轮。
    drop(staying);
    settled(&handle, "cmd-3").await;
    second.end(Exit::Code(0));
    let log = until_logged(&home, handle.id(), |log| counted(log).1 == 2).await;
    settled(&handle, "cmd-4").await;
    assert_eq!(counted(&log), (2, 2), "没人看着：记下了，没开轮");
    assert_eq!(counted(&home.log(handle.id())), (2, 2));
}

#[tokio::test]
async fn nobody_has_subscribed_yet_counts_as_unwatched() {
    // 造会话以后没人订阅过：和载入以后一样当没人看着（`kernel/session.md`「回报」第 6 条）。
    let home = Home::new();
    let (first, second) = (Held::new(&[]), Held::new(&[]));
    let handle = oneshot(&home, &first, &second).await;
    ask(&handle, "cmd-1", say("放一个"))
        .await
        .expect("会话在跑");
    until_logged(&home, handle.id(), |log| {
        log.iter()
            .any(|event| matches!(event.body, Body::TurnEnded(_)))
    })
    .await;
    first.end(Exit::Code(0));
    let log = until_logged(&home, handle.id(), |log| counted(log).1 == 1).await;
    settled(&handle, "cmd-2").await;
    assert_eq!(counted(&log), (1, 1), "只记下");
    assert_eq!(counted(&home.log(handle.id())), (1, 1));
    // 这时有头订阅了，也不因为以前的开轮（2026-09-29 项目主人定）。
    let _late = watch(&handle).await;
    settled(&handle, "cmd-3").await;
    assert_eq!(counted(&home.log(handle.id())), (1, 1));
    second.end(Exit::Code(0));
}

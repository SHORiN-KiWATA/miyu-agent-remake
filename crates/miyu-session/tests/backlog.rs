//! 订阅时补发之前的事件（施工 3-8 六补，`docs/blueprint/session/actor.md` 第 6 条）：补发的那一截和订阅在 actor 的同一步
//! 里拿。补到那一步落了盘的最后一条；之后追加的只从订阅推过来，补的时候日志里已经多了也不读进来：不重不漏。

mod support;

use miyu_kernel::event::Event;
use miyu_session::Pushed;
use miyu_session::testkit::{Play, Script};

use support::*;

/// 推过来的落了盘的事件，照先后。
fn persisted(pushed: &[std::sync::Arc<Pushed>]) -> Vec<Event> {
    pushed
        .iter()
        .flat_map(|pushed| match &**pushed {
            Pushed::Events(events) => events.clone(),
            Pushed::Transient(_) => Vec::new(),
        })
        .collect()
}

#[tokio::test]
async fn the_backlog_ends_where_the_subscription_begins() {
    let home = Home::new();
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let handle = home.create(&script).await;
    let mut first = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut first).await;
    let landed = home.log(handle.id());

    let (mut subscription, backlog) = within("订阅", handle.subscribe_after(0))
        .await
        .expect("会话在跑");
    assert_eq!(
        backlog.upto(),
        landed.len() as u64,
        "补到订阅那一刻的最后一条"
    );
    // 订阅以后、读之前又说一句：落了盘才回应，日志里已经多了。
    ask(&handle, "cmd-2", say("again")).await.expect("会话在跑");
    let read = within("读", backlog.read()).await.expect("读得了");
    assert_eq!(read, landed, "只补到 upto，后来的不读进来");
    let pushed = persisted(&until_turn_ends(&mut subscription).await);
    let log = home.log(handle.id());
    assert_eq!(
        pushed,
        log[landed.len()..],
        "后来的从订阅推过来，从下一条起"
    );
}

#[tokio::test]
async fn the_backlog_starts_after_the_given_seq() {
    let home = Home::new();
    let handle = home.create(&Script::new([Play::Says("一。")])).await;
    let mut first = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut first).await;
    let log = home.log(handle.id());
    let last = log.len() as u64;
    for after in [0, 1, last - 1, last, last + 7] {
        let (_, backlog) = within("订阅", handle.subscribe_after(after))
            .await
            .expect("会话在跑");
        assert_eq!(backlog.upto(), last, "after {after}");
        let read = within("读", backlog.read()).await.expect("读得了");
        let from = usize::try_from(after.min(last)).expect("放得下");
        assert_eq!(read, log[from..], "after {after}：序号大于它的");
    }
}

//! 在后台答的回应照办完那一刻的订阅走（施工 R-7 补）：订阅着的交给它的转发任务，没订阅的、不是给会话的、放下了的直接写；换了
//! 的交给新的；只拿弱的一头，交的一头放下了，转发任务就退得了；放下了的下一次开通道时清掉。

use tokio::sync::mpsc;

use miyu_kernel::id::SessionId;

use super::Later;
use crate::subscriptions::Target;

fn session(n: u32) -> SessionId {
    SessionId::parse(&format!("01900000-0000-7000-8000-{n:012x}")).expect("合写法")
}

#[tokio::test]
async fn a_late_reply_follows_the_subscription_of_that_moment() {
    let later = Later::default();
    let target = Target::Session(session(1));
    let (out, mut written) = mpsc::channel(8);

    later.reply(Some(&target), "a".to_string(), &out).await;
    assert_eq!(
        written.try_recv().ok().as_deref(),
        Some("a"),
        "没订阅的直接写"
    );

    let (replies, mut forwarded) = later.open(session(1));
    later.reply(Some(&target), "b".to_string(), &out).await;
    assert_eq!(
        forwarded.try_recv().ok().as_deref(),
        Some("b"),
        "交给转发任务"
    );
    later.reply(None, "c".to_string(), &out).await;
    assert_eq!(
        written.try_recv().ok().as_deref(),
        Some("c"),
        "不是给会话的直接写"
    );
    assert!(forwarded.try_recv().is_err());

    let (newer, mut renewed) = later.open(session(1));
    drop(replies);
    later.reply(Some(&target), "d".to_string(), &out).await;
    assert_eq!(
        renewed.try_recv().ok().as_deref(),
        Some("d"),
        "换了的交给新的"
    );
    assert_eq!(forwarded.recv().await, None, "旧的那一头没人拿着了");

    drop(newer);
    later.reply(Some(&target), "e".to_string(), &out).await;
    assert_eq!(
        written.try_recv().ok().as_deref(),
        Some("e"),
        "放下了的直接写"
    );
    assert_eq!(renewed.recv().await, None, "只拿弱的：转发任务退得了");
}

#[test]
fn dropped_ones_are_cleared_on_the_next_open() {
    let later = Later::default();
    let kept = later.open(session(1));
    drop(later.open(session(2)));
    let _third = later.open(session(3));
    let left: Vec<SessionId> = later.map().keys().cloned().collect();
    assert_eq!(left, [session(1), session(3)]);
    drop(kept);
}

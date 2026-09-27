//! 订阅：掉过一次队，这个订阅就作废；会话停了，读完剩下的就说停了。

use super::*;
use miyu_kernel::event::{Body, SessionCreated};
use miyu_kernel::id::Seq;
use miyu_kernel::time::Timestamp;

/// 推送一份只有一条事件的。
fn one() -> Arc<Pushed> {
    let created = created();
    Arc::new(Pushed::Events(vec![Event {
        seq: Seq::FIRST,
        at: Timestamp::from_unix_millis(0).expect("在范围里"),
        turn: None,
        by: By::Kernel,
        cause: None,
        body: Body::SessionCreated(created),
    }]))
}

/// 造会话那一条的样子。
fn created() -> SessionCreated {
    let line = r#"{"seq":1,"at":"2026-09-27T07:00:00.000Z","by":{"kind":"kernel"},"kind":"session.created","body":{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}}"#;
    match Event::from_line(line).expect("造会话那一条合写法").body {
        Body::SessionCreated(created) => created,
        other => panic!("应该是造会话那一条：{other:?}"),
    }
}

#[tokio::test]
async fn a_lagging_subscription_is_dropped_for_good() {
    let (pushes, receiver) = broadcast::channel(2);
    let mut slow = Subscription::new(receiver);
    let mut quick = Subscription::new(pushes.subscribe());
    for _ in 0..3 {
        pushes.send(one()).expect("有订阅者");
        quick.next().await.expect("读得上");
    }
    // 慢的那个一份都没读，三份里最早的一份被挤掉了：掉队，以后一直是掉队。
    assert_eq!(slow.next().await, Err(Ended::Lagged));
    assert_eq!(slow.next().await, Err(Ended::Lagged));
    // 会话停了：快的那个读完了，说停了。
    drop(pushes);
    assert_eq!(quick.next().await, Err(Ended::Stopped));
}

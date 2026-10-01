//! 空了告诉我（施工 C-6，`cross-session.md` 第六条第 1、2 款）：`notify_when_idle` 写 `true`、`to` 是别的会话的，那次调用
//! 报 `peer.watch`（整个编号），结果接一句会告诉她；带话的先发，送到了（`sent`、`held`、一模一样的）才订，被拒的整次不订；
//! 不带话的只订、端口一次不问；订子代理、父会话的整次拒、留言也不发；两样都没有的参数不对；这个会话不能找别的会话的照旧拒。
//! 给人看的说法两种语言都换得出字。拆自 `send_message.rs`（行数上限）。

use serde_json::json;

use miyu_kernel::event::PeerWatch;
use miyu_tool::{Delivered, Done, Effect, NotSent};

use super::by_session_id::{Sessions, id, other_session};
use super::{OTHER, Port, Site, THIS, text};
use crate::support::{check, human, readable, said};

/// 订了以后接的那一句：`to` 是她写的原样。
fn watching(to: &str) -> String {
    format!("You will get a notice when {to} is next idle.\n")
}

/// 订了 `OTHER`。
fn watched() -> Vec<Effect> {
    vec![Effect::PeerWatch(PeerWatch { session: id(OTHER) })]
}

/// 调一次，端口照 `answer` 回；交回结果和端口。
async fn call(
    args: serde_json::Value,
    answer: Result<Delivered, NotSent>,
) -> (Done, std::sync::Arc<Port>) {
    let port = Port::new(answer);
    let done = Site::new()
        .done_with_messages_and_sessions(
            "send_message",
            args,
            Some(port.clone()),
            Some(Sessions::new(vec![other_session()])),
        )
        .await;
    (done, port)
}

#[tokio::test]
async fn watching_alone_sends_nothing() {
    let (done, port) = call(
        json!({"to": "9f03b21c", "notify_when_idle": true}),
        Ok(Delivered::Sent),
    )
    .await;
    assert!(!done.error);
    assert_eq!(text(&done), watching("9f03b21c"));
    assert_eq!(done.effects, watched(), "效果里是整个编号");
    assert!(port.asked().is_empty(), "只订不发：那边不开轮");
}

#[tokio::test]
async fn a_message_is_sent_first_and_then_watched() {
    for (answer, first) in [
        (Ok(Delivered::Sent), format!("Message sent to {OTHER}.\n")),
        (
            Ok(Delivered::Held),
            format!(
                "Message saved for {OTHER}. Nobody is watching that one-shot session, so it reads this only when someone continues it.\n"
            ),
        ),
        (
            Err(NotSent::Duplicate),
            format!("{OTHER} already has this exact message.\n"),
        ),
    ] {
        let (done, port) = call(
            json!({"to": OTHER, "message": "跑一遍测试", "notify_when_idle": true}),
            answer,
        )
        .await;
        assert!(!done.error, "{answer:?}");
        assert_eq!(
            text(&done),
            format!("{first}{}", watching(OTHER)),
            "{answer:?}"
        );
        assert_eq!(done.effects, watched(), "{answer:?}：送到了才订");
        assert_eq!(port.asked().len(), 1);
    }
}

#[tokio::test]
async fn a_refused_message_is_not_watched() {
    for why in [NotSent::TooMany, NotSent::InboxFull, NotSent::Undelivered] {
        let (done, _) = call(
            json!({"to": OTHER, "message": "跑一遍测试", "notify_when_idle": true}),
            Err(why),
        )
        .await;
        assert!(done.error, "{why:?}");
        assert!(done.effects.is_empty(), "{why:?}：发话被拒的整次不订");
        assert!(!text(&done).contains("notice"), "{why:?}");
    }
    let (done, port) = call(
        json!({"to": OTHER, "message": "a".repeat(100_001), "notify_when_idle": true}),
        Ok(Delivered::Sent),
    )
    .await;
    assert!(done.error && done.effects.is_empty(), "太长的不订");
    assert!(port.asked().is_empty());
}

#[tokio::test]
async fn only_other_sessions_can_be_watched() {
    for to in ["parent", "j1", "j2.1"] {
        for args in [
            json!({"to": to, "notify_when_idle": true}),
            json!({"to": to, "message": "好了告诉我", "notify_when_idle": true}),
        ] {
            let (done, port) = call(args.clone(), Ok(Delivered::Sent)).await;
            assert!(done.error, "{args}");
            assert_eq!(
                text(&done),
                "notify_when_idle works only for other sessions.\n",
                "{args}"
            );
            assert!(done.effects.is_empty(), "{args}");
            assert!(port.asked().is_empty(), "{args}：整次拒，留言也不发");
        }
    }
    let (done, _) = call(
        json!({"to": THIS, "notify_when_idle": true}),
        Ok(Delivered::Sent),
    )
    .await;
    assert_eq!(text(&done), format!("\"{THIS}\" is this session.\n"));
    assert!(done.effects.is_empty(), "不能订自己");
}

#[tokio::test]
async fn without_a_message_it_must_watch() {
    for args in [
        json!({"to": OTHER}),
        json!({"to": OTHER, "notify_when_idle": false}),
        json!({"to": OTHER, "notify_when_idle": null}),
        json!({"to": OTHER, "message": null}),
    ] {
        let (done, port) = call(args.clone(), Ok(Delivered::Sent)).await;
        assert!(done.error, "{args}");
        assert_eq!(
            text(&done),
            "The arguments are not right: missing field `message`.\n",
            "{args}"
        );
        assert!(port.asked().is_empty(), "{args}");
    }
    let (done, _) = call(
        json!({"to": OTHER, "notify_when_idle": "yes"}),
        Ok(Delivered::Sent),
    )
    .await;
    assert!(done.error, "不是布尔值的参数不对");
}

#[tokio::test]
async fn a_session_that_cannot_reach_others_cannot_watch_them() {
    let port = Port::new(Ok(Delivered::Sent));
    let done = Site::new()
        .done_with_messages(
            "send_message",
            json!({"to": OTHER, "notify_when_idle": true}),
            Some(port.clone()),
        )
        .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "This session cannot message or watch other sessions.\n"
    );
    assert!(done.effects.is_empty());
}

#[tokio::test]
async fn watching_says_something_people_can_read() {
    let mut checked = Vec::new();
    let (only, _) = call(
        json!({"to": "9f03b21c", "notify_when_idle": true}),
        Ok(Delivered::Sent),
    )
    .await;
    check(
        &mut checked,
        human(only),
        said("send_message/watching").with("to", "9f03b21c"),
    );
    let (both, _) = call(
        json!({"to": OTHER, "message": "跑", "notify_when_idle": true}),
        Ok(Delivered::Sent),
    )
    .await;
    check(
        &mut checked,
        human(both),
        said("send_message/sent").with("to", OTHER),
    );
    let (refused, _) = call(
        json!({"to": "j1", "notify_when_idle": true}),
        Ok(Delivered::Sent),
    )
    .await;
    check(
        &mut checked,
        human(refused),
        said("send_message/watch-peers-only"),
    );
    readable(&checked, &["send_message"]);
}

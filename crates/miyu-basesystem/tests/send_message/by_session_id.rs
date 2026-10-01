//! `to` 当会话编号认（施工 C-5，`cross-session.md` 第三条第 1 款）：和 `history` 的 `session` 同一个认法，在这个会话看得到
//! 的主会话（加她自己）里找。拆自 `send_message.rs`（照 `spawn/renamed.rs` 的先例，那个文件超过了行数上限）。

use std::sync::Arc;

use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;
use miyu_tool::{Delivered, Listing, MainSession, Opening, Recipient, SessionsPort, Stop};

use super::{OTHER, Port, Site, THIS, text, to};
use crate::support::{check, human, readable, said};

/// 假的列会话端口：她自己是 `THIS`，看得到的只有 `others`。
pub(super) struct Sessions {
    this: SessionId,
    others: Vec<MainSession>,
}

impl Sessions {
    pub(super) fn new(others: Vec<MainSession>) -> Arc<Sessions> {
        Arc::new(Sessions {
            this: id(THIS),
            others,
        })
    }
}

impl SessionsPort for Sessions {
    fn this(&self) -> &SessionId {
        &self.this
    }

    fn list<'a>(&'a self, _stop: &'a Stop) -> Listing<'a> {
        let others = self.others.clone();
        Box::pin(async move { Ok(others) })
    }

    /// `send_message` 不开别的会话的日志：那是 `history` 的事。
    fn open<'a>(&'a self, _session: &'a SessionId) -> Opening<'a> {
        Box::pin(async { Err("not reached in this test".to_string()) })
    }
}

pub(super) fn id(text: &str) -> SessionId {
    SessionId::parse(text).expect("合写法")
}

pub(super) fn other_session() -> MainSession {
    MainSession {
        id: id(OTHER),
        title: "修 CI".to_string(),
        cwd: "~/src".to_string(),
        busy: false,
        last_active: Timestamp::parse("2026-10-01T09:00:00.000Z").expect("合写法"),
    }
}

#[tokio::test]
async fn it_sends_to_another_session_found_by_its_id() {
    let site = Site::new();
    let messages = Port::new(Ok(Delivered::Sent));
    let sessions = Sessions::new(vec![other_session()]);
    for written in [OTHER, "9f03b21c"] {
        let done = site
            .done_with_messages_and_sessions(
                "send_message",
                to(written),
                Some(messages.clone()),
                Some(sessions.clone()),
            )
            .await;
        assert!(!done.error, "{written}");
        assert_eq!(text(&done), format!("Message sent to {written}.\n"));
    }
    assert_eq!(
        messages.asked(),
        [
            (
                Recipient::Session(id(OTHER)),
                "Which file should I change?\nOnly one.".to_string()
            ),
            (
                Recipient::Session(id(OTHER)),
                "Which file should I change?\nOnly one.".to_string()
            ),
        ],
        "整个编号、短编号都认得到同一个会话"
    );
}

#[tokio::test]
async fn a_recipient_that_looks_like_neither_parent_nor_a_job_id_is_tried_as_a_session() {
    let site = Site::new();
    let port = Port::new(Ok(Delivered::Sent));
    let sessions = Sessions::new(vec![other_session()]);
    // 没有列会话的端口（子会话、场所会话）：不去找，直接拒，端口一次都不问。
    for recipient in [
        "sibling", "J1", "j0", "j1.0", "j1.", "j.1", " j1", "Parent", "",
    ] {
        let done = site
            .done_with_messages("send_message", to(recipient), Some(port.clone()))
            .await;
        assert!(done.error);
        assert_eq!(
            text(&done),
            "This session cannot message or watch other sessions.\n",
            "{recipient:?}"
        );
    }
    assert!(port.asked().is_empty(), "不去找：端口一次都不问");
    // 有列会话的端口的：当会话编号认，找不到的说找不到。
    let done = site
        .done_with_messages_and_sessions(
            "send_message",
            to("sibling"),
            Some(port.clone()),
            Some(sessions.clone()),
        )
        .await;
    assert!(done.error);
    assert_eq!(text(&done), "No session has the id \"sibling\".\n");
    assert!(port.asked().is_empty());
}

#[tokio::test]
async fn an_ambiguous_or_own_session_id_is_rejected_without_sending() {
    let site = Site::new();
    let port = Port::new(Ok(Delivered::Sent));
    let twin = MainSession {
        id: SessionId::parse("0192f3a0-3333-7abc-8def-00119f03b21c").expect("合写法"),
        ..other_session()
    };
    let sessions = Sessions::new(vec![other_session(), twin]);
    let done = site
        .done_with_messages_and_sessions(
            "send_message",
            to("9f03b21c"),
            Some(port.clone()),
            Some(sessions.clone()),
        )
        .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "\"9f03b21c\" matches more than one session. Use the full id.\n"
    );
    let done = site
        .done_with_messages_and_sessions(
            "send_message",
            to(THIS),
            Some(port.clone()),
            Some(sessions.clone()),
        )
        .await;
    assert!(done.error);
    assert_eq!(text(&done), format!("\"{THIS}\" is this session.\n"));
    assert!(port.asked().is_empty(), "两种都拒绝，不去送");
}

#[tokio::test]
async fn every_session_outcome_says_something_people_can_read() {
    let site = Site::new();
    let mut checked = Vec::new();
    let sessions = Sessions::new(vec![other_session()]);
    let no_session = site
        .done_with_messages_and_sessions(
            "send_message",
            to("sibling"),
            Some(Port::new(Ok(Delivered::Sent))),
            Some(sessions.clone()),
        )
        .await;
    check(
        &mut checked,
        human(no_session),
        said("send_message/no-session").with("to", "sibling"),
    );
    let twin = MainSession {
        id: SessionId::parse("0192f3a0-3333-7abc-8def-00119f03b21c").expect("合写法"),
        ..other_session()
    };
    let ambiguous_sessions = Sessions::new(vec![other_session(), twin]);
    let ambiguous = site
        .done_with_messages_and_sessions(
            "send_message",
            to("9f03b21c"),
            Some(Port::new(Ok(Delivered::Sent))),
            Some(ambiguous_sessions),
        )
        .await;
    check(
        &mut checked,
        human(ambiguous),
        said("send_message/ambiguous").with("to", "9f03b21c"),
    );
    let itself = site
        .done_with_messages_and_sessions(
            "send_message",
            to(THIS),
            Some(Port::new(Ok(Delivered::Sent))),
            Some(sessions.clone()),
        )
        .await;
    check(
        &mut checked,
        human(itself),
        said("send_message/self").with("to", THIS),
    );
    readable(&checked, &["send_message"]);
}

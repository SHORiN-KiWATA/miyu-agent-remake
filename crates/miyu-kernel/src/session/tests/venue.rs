//! 场所的消息和桥记的事件（施工 O-13 上，`docs/blueprint/chat.md` 第七条第 2、3 条）：旁听的消息只记下，不开回合，回合
//! 进行中也不排进这一轮、说完了不接着开；`Append` 记的事件不带回合编号、不开回合，回合中途照记。

use super::executor::*;
use super::*;
use crate::event::{Media, MediaKind, VenueMessage, VenueRecalled};
use crate::session::{Appended, ExtEvent};

/// 一条场所的消息：平台编号 `msg`，旁听的照 `ambient`。
fn venue(msg: &str, ambient: bool) -> VenueMessage {
    VenueMessage {
        msg: msg.to_string(),
        name: Some("阿杰".to_string()),
        media: vec![Media {
            kind: MediaKind::File,
            id: "f1".to_string(),
            name: Some("报告.pdf".to_string()),
        }],
        ambient,
        ..VenueMessage::default()
    }
}

/// 编号是 `n` 的命令：alice 发一条带 `venue` 的话。
fn said(n: u64, words: &str, venue: VenueMessage) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Send {
            blocks: vec![Block::Text(Text {
                text: words.to_string(),
            })],
            urgent: false,
            venue: Some(venue),
        },
    })
}

/// 编号是 `n` 的命令：记一条 `appended`。
fn append(n: u64, appended: Appended) -> Input {
    Input::Command(Received {
        id: id(n),
        by: alice(),
        at: at(n % 60),
        command: Command::Append { event: appended },
    })
}

fn ext(kind: &str) -> Appended {
    let event = ExtEvent::new(kind, serde_json::json!({"step": 1})).expect("ext. 开头的");
    Appended::Ext(event)
}

#[test]
fn an_ambient_message_is_kept_without_a_turn() {
    let mut session = session();
    let actions = session.handle(said(1, "今天谁值班", venue("8810", true)));
    let events = appended_events(&actions);
    assert_eq!(events.len(), 1, "只记下，不开回合：{events:?}");
    assert_eq!(events[0].turn, None);
    let Body::MessageUser(message) = &events[0].body else {
        panic!("应该是 message.user：{events:?}");
    };
    assert_eq!(message.venue, Some(venue("8810", true)), "原样记下");
    let actions = session.handle(stored(2));
    assert!(actions.contains(&accepted_reply(1, &[2])), "{actions:?}");
    assert!(hooks(&actions).is_empty(), "不开回合");
    // 不旁听的场所消息照常开回合。
    assert_eq!(
        appended(&session.handle(said(3, "@她 你来", venue("8811", false)))),
        seqs(&[3, 4, 5, 6])
    );
}

#[test]
fn an_ambient_message_during_a_turn_stays_out_of_it() {
    let mut session = asking();
    let actions = allowing(&mut session, said(2, "我吧", venue("8812", true)));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6]));
    assert_eq!(events[0].turn, None, "不带这一轮");
    let actions = answer(&mut session, 5, "好");
    let events = appended_events(&actions);
    assert!(
        events
            .iter()
            .all(|event| !matches!(event.body, Body::TurnStarted(_))),
        "说完了不接着开一轮：{events:?}"
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event.body, Body::TurnEnded(_))),
        "{events:?}"
    );
}

#[test]
fn appended_events_carry_no_turn_and_open_nothing() {
    let mut session = session();
    let actions = session.handle(append(1, ext("ext.onebot.chat.decided")));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[2]));
    assert_eq!(events[0].turn, None);
    assert_eq!(events[0].body.kind(), "ext.onebot.chat.decided");
    let actions = session.handle(stored(2));
    assert!(actions.contains(&accepted_reply(1, &[2])), "{actions:?}");
    assert!(hooks(&actions).is_empty(), "不开回合");

    let mut session = asking();
    let recalled = Appended::Recalled(VenueRecalled {
        msg: "8813".to_string(),
        by: crate::id::ExternalId::parse("qq:20017").expect("合写法"),
    });
    let actions = allowing(&mut session, append(2, recalled));
    let events = appended_events(&actions);
    assert_eq!(appended(&actions), seqs(&[6]), "回合中途照记");
    assert_eq!(events[0].turn, None);
    assert_eq!(events[0].body.kind(), "venue.recalled");
}

#[test]
fn only_ext_kinds_are_free_form() {
    assert!(ExtEvent::new("ext.onebot.venues.queued", serde_json::json!({})).is_some());
    for kind in ["message.user", "extra.thing", "ext", "venue.recalled"] {
        assert!(
            ExtEvent::new(kind, serde_json::json!({})).is_none(),
            "{kind} 不是 ext. 开头的"
        );
    }
}

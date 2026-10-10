//! 叫她做的那条（施工 O-31，`onebot.md` 第一条「平台工具（一）」第 3 条）：投影另记人说的话的引用和 @；主线这一轮她回的那一条
//! 交出来（`turn.joined` 并进来的换上，这一轮完了没有）；引用的是她的、是谁的、桥不认识的；终端管理员照他说过的话认。

use serde_json::json;

use super::super::platform::{Origin, Quote};
use super::Projection;
use super::tests::{ended, event, joined, qq, started, take_all};

/// 号是 `user` 的人说的第 `seq` 条：引用 `reply_to`、@ 了 `mentions`；`admin` 的带 `account`。
fn quoting(
    seq: u64,
    user: i64,
    reply_to: Option<&str>,
    mentions: &[i64],
    admin: bool,
) -> miyu_kernel::event::Event {
    let mut by = json!({"kind": "external", "venue": "qq:group:5", "id": format!("qq:{user}")});
    if admin {
        by["account"] = json!("admin");
    }
    let mut venue = json!({"msg": seq.to_string(), "ambient": true});
    if let Some(reply_to) = reply_to {
        venue["reply_to"] = json!(reply_to);
    }
    if !mentions.is_empty() {
        let mentions: Vec<String> = mentions.iter().map(|one| format!("qq:{one}")).collect();
        venue["mentions"] = json!(mentions);
    }
    let body = json!({"blocks": [{"type": "text", "text": "嗨"}], "venue": venue});
    event(seq, 0, "message.user", None, by, body)
}

/// 她发出去的一段：平台编号 `msg`。
fn delivered(seq: u64, msg: &str) -> miyu_kernel::event::Event {
    let body = json!({"line": "01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91", "turn": 1, "to": [], "msg": msg, "text": "在。"});
    event(
        seq,
        0,
        "venue.delivered",
        None,
        json!({"kind": "module", "id": "onebot"}),
        body,
    )
}

#[test]
fn the_origin_is_the_message_she_answers_with_its_quote_and_mentions() {
    let mut projection = Projection::new(0);
    take_all(
        &mut projection,
        vec![
            quoting(1, 10001, None, &[], true),
            delivered(2, "90001"),
            quoting(3, 20002, Some("1"), &[20003], false),
        ],
    );
    assert_eq!(projection.origin(), None, "主线不在跑");
    take_all(&mut projection, vec![started(4, 0, &[3])]);
    assert_eq!(
        projection.origin(),
        Some(Origin {
            sender: qq(20002),
            quote: Some(Quote {
                msg: "1".to_string(),
                mine: false,
                sender: Some(qq(10001)),
            }),
            mentions: vec![qq(20003)],
        })
    );
    // 并进来的换上：引用她的。
    take_all(
        &mut projection,
        vec![
            quoting(5, 20003, Some("90001"), &[], false),
            joined(6, 4, &[5]),
        ],
    );
    assert_eq!(
        projection.origin(),
        Some(Origin {
            sender: qq(20003),
            quote: Some(Quote {
                msg: "90001".to_string(),
                mine: true,
                sender: None,
            }),
            mentions: Vec::new(),
        })
    );
    take_all(&mut projection, vec![ended(7, 4)]);
    assert_eq!(projection.origin(), None, "这一轮完了");
    // 引用桥不认识的：不知道是谁的。
    take_all(
        &mut projection,
        vec![
            quoting(8, 20002, Some("777"), &[], false),
            started(9, 0, &[8]),
        ],
    );
    let quote = projection.origin().and_then(|origin| origin.quote);
    assert_eq!(
        quote,
        Some(Quote {
            msg: "777".to_string(),
            mine: false,
            sender: None,
        })
    );
}

#[test]
fn the_terminal_admin_is_known_by_what_he_said_here() {
    let mut projection = Projection::new(0);
    take_all(
        &mut projection,
        vec![
            quoting(1, 20002, None, &[], false),
            quoting(2, 10001, None, &[], true),
        ],
    );
    assert!(projection.is_admin(&qq(10001)));
    assert!(!projection.is_admin(&qq(20002)));
    assert!(!projection.is_admin(&qq(30003)), "没说过话的认不出");
}

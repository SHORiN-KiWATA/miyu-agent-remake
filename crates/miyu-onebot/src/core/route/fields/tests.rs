//! 场所的格照核心的写法洗（`onebot.md` 第一条「群消息」第 6 条，「施工时定的」第 63 条）：名字去掉控制字符、截到上限，空白
//! 的不写；引用、带的东西的编号不合的那一格、那一样不记，消息照记；假的开关不写。

use serde_json::json;

use super::{Flags, MEDIA_NAME, NAME, fields};
use crate::onebot::{Media, MediaKind, Posted, Segments, person};

/// 一条消息：名字是 `name`，认出来的段是 `segments`。
fn posted(name: Option<&str>, segments: Segments) -> Posted {
    Posted {
        bot: 30003,
        user: 20002,
        message_id: 9,
        time: 0,
        name: name.map(str::to_string),
        text: String::new(),
        segments,
    }
}

/// 一样带的东西。
fn media(id: &str, name: Option<&str>) -> Media {
    Media {
        kind: MediaKind::File,
        id: id.to_string(),
        name: name.map(str::to_string),
    }
}

#[test]
fn only_what_is_there_is_written() {
    assert_eq!(
        fields(&posted(None, Segments::default()), &[], Flags::default()),
        json!({"msg": "9"})
    );
    let all = Flags {
        mentions_me: true,
        ambient: true,
        asleep: true,
        show_ids: true,
    };
    let segments = Segments {
        reply_to: Some("8".to_string()),
        mentions_all: true,
        media: vec![media("f-1", Some("a.pdf"))],
        ..Segments::default()
    };
    let mentions = [person(20003).expect("拼得出")];
    assert_eq!(
        fields(&posted(Some("小林"), segments), &mentions, all),
        json!({
            "msg": "9", "name": "小林", "reply_to": "8", "mentions": ["qq:20003"], "mentions_me": true,
            "mentions_all": true, "media": [{"kind": "file", "id": "f-1", "name": "a.pdf"}], "ambient": true,
            "asleep": true, "show_ids": true,
        })
    );
}

#[test]
fn names_lose_control_characters_and_are_cut() {
    let long = "名".repeat(NAME + 6);
    let written = |name: &str| {
        fields(
            &posted(Some(name), Segments::default()),
            &[],
            Flags::default(),
        )
    };
    assert_eq!(written(&long)["name"], "名".repeat(NAME));
    assert_eq!(written("小\u{0}林\n")["name"], "小林");
    assert_eq!(written(" 小林 ")["name"], " 小林 ", "照原样，只去控制字符");
    for blank in ["", "  ", "\u{7}\u{1b}", " \n "] {
        assert!(written(blank).get("name").is_none(), "{blank:?}");
    }
    let segments = Segments {
        media: vec![
            media("f-1", Some(&"文".repeat(MEDIA_NAME + 1))),
            media("f-2", Some(" \t")),
        ],
        ..Segments::default()
    };
    let written = fields(&posted(None, segments), &[], Flags::default());
    assert_eq!(
        written["media"],
        json!([{"kind": "file", "id": "f-1", "name": "文".repeat(MEDIA_NAME)}, {"kind": "file", "id": "f-2"}])
    );
}

#[test]
fn ids_that_do_not_fit_are_left_out_and_the_message_is_kept() {
    let longest = "1".repeat(128);
    for (reply_to, kept) in [
        (longest.clone(), true),
        ("1".repeat(129), false),
        ("8\n".to_string(), false),
        (String::new(), false),
    ] {
        let segments = Segments {
            reply_to: Some(reply_to.clone()),
            ..Segments::default()
        };
        let written = fields(&posted(None, segments), &[], Flags::default());
        assert_eq!(written.get("reply_to").is_some(), kept, "{reply_to:?}");
        assert_eq!(written["msg"], "9");
    }
    let segments = Segments {
        media: vec![
            media(&"x".repeat(129), None),
            media(&longest, None),
            media("a\u{0}", None),
        ],
        ..Segments::default()
    };
    let written = fields(&posted(None, segments), &[], Flags::default());
    assert_eq!(written["media"], json!([{"kind": "file", "id": longest}]));
    let segments = Segments {
        media: vec![media(&"x".repeat(129), None)],
        ..Segments::default()
    };
    assert!(
        fields(&posted(None, segments), &[], Flags::default())
            .get("media")
            .is_none(),
        "一样都不剩的不写"
    );
}

//! 认消息段（施工 O-22，`onebot.md` 第一条「群消息」第 2 条）：每一种段记成什么；带的东西的编号取哪一格、整数也认；几段引用
//! 只认第一段；@ 同一个人两次只算一次；CQ 字符串只读字。

use serde_json::{Value, json};

use miyu_onebot::onebot::{Media, MediaKind, Piece, Segments, segments};

/// 一样带的东西。
fn media(kind: MediaKind, id: &str, name: Option<&str>) -> Media {
    Media {
        kind,
        id: id.to_string(),
        name: name.map(str::to_string),
    }
}

/// 一段字。
fn words(text: &str) -> Piece {
    Piece::Text(text.to_string())
}

/// 只有一段 `segment` 的消息认出来带的东西。
fn only(segment: Value) -> Vec<Media> {
    segments(&json!([segment])).media
}

#[test]
fn text_at_reply_and_placeholders_make_the_body() {
    let read = segments(&json!([
        {"type": "reply", "data": {"id": "41"}},
        {"type": "at", "data": {"qq": "20002"}},
        {"type": "text", "data": {"text": " 看 "}},
        {"type": "at", "data": {"qq": 20003}},
        {"type": "reply", "data": {"id": "42"}},
        {"type": "at", "data": {"qq": "all"}},
        {"type": "forward", "data": {"id": "fw"}},
        {"type": "json", "data": {"data": "{}"}},
        {"type": "xml", "data": {"data": "<x/>"}},
        {"type": "at", "data": {"qq": "20002"}},
        {"type": "at", "data": {"qq": "nobody"}},
        {"type": "poke", "data": {"type": "1", "id": "1"}},
        {"type": "dice", "data": {"result": "3"}},
    ]));
    assert_eq!(
        read,
        Segments {
            pieces: vec![
                Piece::At(20002),
                words(" 看 "),
                Piece::At(20003),
                words("[forward]"),
                words("[card]"),
                words("[card]"),
                Piece::At(20002),
            ],
            reply_to: Some("41".to_string()),
            mentions_all: true,
            media: Vec::new(),
        },
        "几段引用只认第一段，@全体不进正文，认不出号的 @、戳一戳、骰子不记"
    );
    assert_eq!(read.at(), [20002, 20003], "照先后、不重");
    assert_eq!(
        segments(&json!([{"type": "reply", "data": {"id": 7}}])).reply_to,
        Some("7".to_string()),
        "整数的编号写成字"
    );
}

#[test]
fn each_kind_of_media_is_named_right() {
    assert_eq!(
        only(
            json!({"type": "image", "data": {"file": "a.jpg", "sub_type": 0, "summary": "", "url": "https://x"}})
        ),
        [media(MediaKind::Image, "a.jpg", None)]
    );
    assert_eq!(
        only(
            json!({"type": "image", "data": {"file": "b.gif", "sub_type": "1", "summary": "[动画表情]"}})
        ),
        [media(MediaKind::Sticker, "b.gif", None)],
        "表情包是表情，「[动画表情]」不是表情的字"
    );
    assert_eq!(
        only(
            json!({"type": "image", "data": {"file": "x.gif", "emoji_id": "e1", "summary": "[吃瓜]"}})
        ),
        [media(MediaKind::Sticker, "x.gif", Some("[吃瓜]"))],
        "NapCat 把商城表情发成图片"
    );
    assert_eq!(
        only(
            json!({"type": "mface", "data": {"emoji_id": "m1", "emoji_package_id": 9, "summary": "[比心]"}})
        ),
        [media(MediaKind::Sticker, "m1", Some("[比心]"))]
    );
    assert_eq!(
        only(json!({"type": "face", "data": {"id": "14", "raw": {"faceText": "/微笑"}}})),
        [media(MediaKind::Sticker, "14", Some("/微笑"))]
    );
    assert_eq!(
        only(json!({"type": "face", "data": {"id": 178, "raw": {"faceText": "  "}}})),
        [media(MediaKind::Sticker, "178", None)],
        "没有字的小黄脸不写名字"
    );
    assert_eq!(
        only(json!({"type": "record", "data": {"file": "v.amr", "url": "https://x"}})),
        [media(MediaKind::Voice, "v.amr", None)]
    );
    assert_eq!(
        only(json!({"type": "video", "data": {"file": "c.mp4", "file_id": "vid"}})),
        [media(MediaKind::Video, "vid", None)],
        "file_id 在 file 前面"
    );
    assert_eq!(
        only(json!({"type": "file", "data": {"file": "排班.pdf", "file_id": "/f-1"}})),
        [media(MediaKind::File, "/f-1", Some("排班.pdf"))]
    );
    assert_eq!(
        only(
            json!({"type": "file", "data": {"name": "a.txt", "file_name": "b.txt", "file": "c.txt"}})
        ),
        [media(MediaKind::File, "c.txt", Some("a.txt"))],
        "名字照 name、file_name、file 的先后"
    );
}

#[test]
fn media_without_an_id_is_not_recorded() {
    for segment in [
        json!({"type": "image", "data": {"summary": "[图片]"}}),
        json!({"type": "image", "data": {"file": ""}}),
        json!({"type": "record", "data": {"file": null}}),
        json!({"type": "file", "data": {"name": "a.txt"}}),
        json!({"type": "video"}),
    ] {
        assert_eq!(only(segment.clone()), [], "{segment}");
    }
}

#[test]
fn a_cq_string_is_read_as_words_only() {
    let read = segments(&json!(
        "[CQ:reply,id=1][CQ:at,qq=30003] 在&#91;吗&#93;[CQ:image,file=a.png]"
    ));
    assert_eq!(
        read,
        Segments {
            pieces: vec![words(" 在[吗]")],
            ..Segments::default()
        }
    );
    assert_eq!(
        segments(&json!("[CQ:image,file=a.png]")),
        Segments::default()
    );
    assert_eq!(segments(&json!(null)), Segments::default());
}

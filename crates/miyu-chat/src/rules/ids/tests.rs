//! 编号的拼和解（`chat.md` 第七条第 1 条）：场所和平台上的人拼出来的样子、解得回原样；号里带 `:` 的；平台不是名字的、
//! 种类不认识的、号是空的或带空白的、太长的，拼不出也解不出。

use miyu_kernel::id::{ExternalId, VenueId};

use super::super::test_support::{group, private};
use super::super::{Venue, VenueKind};
use super::{parse_person, person};

/// 读一个场所编号，断定合内核的写法。
fn venue_id(text: &str) -> VenueId {
    VenueId::parse(text).expect(text)
}

/// 读一个平台上的人的编号，断定合内核的写法。
fn external(text: &str) -> ExternalId {
    ExternalId::parse(text).expect(text)
}

#[test]
fn venue_ids_are_platform_kind_number() {
    assert_eq!(group("123456").id(), Ok(venue_id("qq:group:123456")));
    assert_eq!(private("10002").id(), Ok(venue_id("qq:private:10002")));
}

#[test]
fn venue_ids_parse_back() {
    for venue in [group("123456"), private("10002"), group("a:b"), group("x")] {
        let id = venue.id().expect("拼得出");
        assert_eq!(Venue::parse(&id), Some(venue));
    }
    let tg = Venue {
        platform: "tg-2".to_string(),
        kind: VenueKind::Private,
        id: "-100:7".to_string(),
    };
    assert_eq!(Venue::parse(&venue_id("tg-2:private:-100:7")), Some(tg));
}

#[test]
fn bad_venue_ids_do_not_parse() {
    for text in [
        "qq",
        "qq:group",
        "qq:group:",
        ":group:1",
        "QQ:group:1",
        "qq:Group:1",
        "qq:channel:1",
        "qq:group:1 2",
        "qq:group: 1",
        "qq:private:\u{3000}",
        "1qq:group:1",
    ] {
        assert_eq!(Venue::parse(&venue_id(text)), None, "{text}");
    }
}

#[test]
fn bad_venues_have_no_id() {
    let mut upper = group("1");
    upper.platform = "QQ".to_string();
    let mut colon = group("1");
    colon.platform = "q:q".to_string();
    let empty = group("");
    let spaced = group("1 2");
    let long = group(&"9".repeat(120));
    for venue in [upper, colon, empty, spaced, long] {
        let error = venue.id().expect_err(&format!("{venue:?}"));
        assert_eq!(error.what, "venue");
    }
    // 正好 128 字节的拼得出：`qq:group:` 是 9 个字节。
    assert!(group(&"9".repeat(119)).id().is_ok());
}

#[test]
fn people_are_platform_number() {
    assert_eq!(person("qq", "10002"), Ok(external("qq:10002")));
    assert_eq!(person("qq", "a:b"), Ok(external("qq:a:b")));
    let id = external("qq:10002");
    assert_eq!(parse_person(&id), Some(("qq", "10002")));
    let id = external("qq:a:b");
    assert_eq!(
        parse_person(&id),
        Some(("qq", "a:b")),
        "在第一个 `:` 处切开"
    );
}

#[test]
fn bad_people() {
    for (platform, number) in [
        ("QQ", "1"),
        ("", "1"),
        ("q:q", "1"),
        ("qq", ""),
        ("qq", "1 2"),
        ("qq", &"9".repeat(126)),
    ] {
        let error = person(platform, number).expect_err(&format!("{platform}:{number}"));
        assert_eq!(error.what, "external identity");
    }
    assert!(person("qq", &"9".repeat(125)).is_ok(), "正好 128 字节");
    for text in ["qq", "qq:", ":1", "QQ:1", "qq:1 2", "qq:\t1"] {
        let Ok(id) = ExternalId::parse(text) else {
            continue;
        };
        assert_eq!(parse_person(&id), None, "{text}");
    }
}

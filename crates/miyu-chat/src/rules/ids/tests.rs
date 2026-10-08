//! 编号的拼和解（`chat.md` 第七条第 1 条）：场所和平台上的人拼出来的样子、解得回原样；号里带 `:` 的；平台不是名字的、
//! 种类不认识的、号是空的或带空白的、太长的，拼不出也解不出；`Venue::new` 造不出这些，造得出的读得回原样（O-12 下）。

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
    assert_eq!(group("123456").id(), &venue_id("qq:group:123456"));
    assert_eq!(private("10002").id(), &venue_id("qq:private:10002"));
}

#[test]
fn venues_read_back_what_they_were_made_of() {
    let venue = Venue::new("tg-2", VenueKind::Private, "-100:7").expect("合写法");
    assert_eq!(venue.platform(), "tg-2");
    assert_eq!(venue.kind(), VenueKind::Private);
    assert_eq!(venue.number(), "-100:7");
    assert_eq!(venue.id(), &venue_id("tg-2:private:-100:7"));
}

#[test]
fn venue_ids_parse_back() {
    for venue in [group("123456"), private("10002"), group("a:b"), group("x")] {
        assert_eq!(Venue::parse(venue.id()).as_ref(), Some(&venue));
    }
    let tg = Venue::new("tg-2", VenueKind::Private, "-100:7").expect("合写法");
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
fn bad_venues_cannot_be_made() {
    let long = "9".repeat(120);
    for (platform, number) in [
        ("QQ", "1"),
        ("", "1"),
        ("q:q", "1"),
        ("1qq", "1"),
        ("qq", ""),
        ("qq", "1 2"),
        ("qq", "1\n"),
        ("qq", long.as_str()),
    ] {
        for kind in [VenueKind::Group, VenueKind::Private] {
            let error = Venue::new(platform, kind, number)
                .expect_err(&format!("{platform}:{kind:?}:{number}"));
            assert_eq!(error.what, "venue");
        }
    }
    // 正好 128 字节的造得出：`qq:group:` 是 9 个字节；`qq:private:` 是 11 个，多两个字节就造不出。
    assert!(Venue::new("qq", VenueKind::Group, &"9".repeat(119)).is_ok());
    assert!(Venue::new("qq", VenueKind::Private, &"9".repeat(117)).is_ok());
    assert!(Venue::new("qq", VenueKind::Private, &"9".repeat(118)).is_err());
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

//! 时钟不往回走；会话编号是 UUIDv7，前 48 位是那一刻的毫秒。

use super::*;

#[test]
fn the_clock_never_goes_back() {
    let mut clock = Clock::default();
    assert_eq!(clock.at(5_000).unix_millis(), 5_000);
    // 系统时间往回拨了：照上一次的。
    assert_eq!(clock.at(4_000).unix_millis(), 5_000);
    assert_eq!(clock.at(6_000).unix_millis(), 6_000);
    // 系统时间在 1970 年以前：当 0，也不往回走。
    assert_eq!(clock.at(-1).unix_millis(), 6_000);
    // 走出了范围：停在范围里的最后一刻。
    assert_eq!(clock.at(i64::MAX).unix_millis(), LAST);
    // 真的系统时间在范围里，而且不比上一次早。
    let mut clock = Clock::default();
    let first = clock.now();
    assert!(clock.now() >= first);
}

#[test]
fn a_new_id_is_a_uuid_v7_of_that_moment() {
    let at = Timestamp::parse("2026-09-27T07:00:00.123Z").expect("时刻合写法");
    let id = new_id(at);
    let text = id.as_str();
    // 前 48 位是毫秒：十二位十六进制，照时间排。
    let millis = u64::try_from(at.unix_millis()).expect("1970 年以后");
    assert_eq!(text[..8], format!("{:08x}", millis >> 16), "{text}");
    assert_eq!(text[9..13], format!("{:04x}", millis & 0xffff), "{text}");
    // 版本是 7，变体是 10xx。
    assert_eq!(&text[14..15], "7", "{text}");
    assert!(matches!(&text[19..20], "8" | "9" | "a" | "b"), "{text}");
    // 同一刻的两个也不一样。
    assert_ne!(new_id(at), id);
}

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
    // 自己带一份计数器：共用的那份可能刚被别的测试拿更晚的时刻用过，会照上一次的毫秒。
    let order = Mutex::new(ContextV7::new());
    let id = id_with(at, &order);
    let text = id.as_str();
    // 前 48 位是毫秒：十二位十六进制，照时间排。
    let millis = u64::try_from(at.unix_millis()).expect("1970 年以后");
    assert_eq!(text[..8], format!("{:08x}", millis >> 16), "{text}");
    assert_eq!(text[9..13], format!("{:04x}", millis & 0xffff), "{text}");
    // 版本是 7，变体是 10xx。
    assert_eq!(&text[14..15], "7", "{text}");
    assert!(matches!(&text[19..20], "8" | "9" | "a" | "b"), "{text}");
    // 同一刻的两个也不一样，后造的排在后面。
    let next = id_with(at, &order);
    assert_ne!(next, id);
    assert!(next.as_str() > id.as_str());
    // 往回拨了：照上一次的毫秒，不排到前面去。
    let earlier = Timestamp::parse("2026-09-27T06:59:59.000Z").expect("时刻合写法");
    assert!(id_with(earlier, &order).as_str() > next.as_str());
}

#[test]
fn ids_made_in_the_same_millisecond_keep_their_order() {
    // 同一刻连造一千个：一个比一个大，列会话时照造的先后（施工 3-9 补）。
    let at = Timestamp::parse("2026-09-27T07:00:00.123Z").expect("时刻合写法");
    let ids: Vec<SessionId> = (0..1000).map(|_| new_id(at)).collect();
    for pair in ids.windows(2) {
        assert!(
            pair[0].as_str() < pair[1].as_str(),
            "{} 应该排在 {} 前面",
            pair[0].as_str(),
            pair[1].as_str()
        );
    }
}

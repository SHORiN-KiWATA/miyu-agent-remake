//! 时间的测试：图纸上的例子；几个标准时刻；1600 到 2400 年一天一天数过去，和换算公式对得上；
//! 每一种坏写法各一个。时区的写法和范围；当地的钟点：跨日、跨月、跨年、闰日、1970 年以前、
//! 负的时区、差半小时的时区，一周七天的写法。

use super::*;

fn at(text: &str) -> i64 {
    Timestamp::parse(text).unwrap().unix_millis()
}

#[test]
fn sample_from_the_drawing_round_trips() {
    let json = r#""2026-09-25T07:04:05.123Z""#;
    let t: Timestamp = serde_json::from_str(json).unwrap();
    assert_eq!(t.unix_millis(), 1_790_319_845_123);
    assert_eq!(serde_json::to_string(&t).unwrap(), json);
}

#[test]
fn well_known_moments() {
    for (text, ms) in [
        ("1970-01-01T00:00:00.000Z", 0),
        ("1969-12-31T23:59:59.999Z", -1),
        ("2000-02-29T00:00:00.000Z", 951_782_400_000),
        ("0000-01-01T00:00:00.000Z", MIN),
        ("9999-12-31T23:59:59.999Z", MAX),
    ] {
        assert_eq!(at(text), ms, "{text}");
        assert_eq!(Timestamp::from_unix_millis(ms).unwrap().to_string(), text);
    }
}

#[test]
fn years_outside_0000_to_9999_are_refused() {
    assert_eq!(Timestamp::from_unix_millis(MIN - 1), None);
    assert_eq!(Timestamp::from_unix_millis(MAX + 1), None);
}

#[test]
fn leap_years_follow_the_gregorian_rule() {
    for (year, leap) in [
        (1600, true),
        (1700, false),
        (1900, false),
        (2000, true),
        (2024, true),
        (2026, false),
        (2100, false),
        (2400, true),
    ] {
        assert_eq!(is_leap(year), leap, "{year}");
    }
}

/// 用最笨的办法一天一天往后数，和换算公式比对。1970-01-01 是第 0 天，这是锚点。
#[test]
fn every_day_from_1600_to_2400_matches_counting() {
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    let (mut year, mut month, mut day) = (1600, 1, 1);
    let mut count = days_from_civil(1600, 1, 1);
    while year < 2400 {
        assert_eq!(
            days_from_civil(year, month, day),
            count,
            "{year}-{month}-{day}"
        );
        assert_eq!(civil_from_days(count), (year, month, day));
        count += 1;
        day += 1;
        if day > days_in_month(year, month) {
            day = 1;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }
    }
}

#[test]
fn bad_text_is_refused_with_a_reason() {
    for (text, why) in [
        ("2026-02-29T00:00:00.000Z", "没有这一天"),
        ("1900-02-29T00:00:00.000Z", "没有这一天"),
        ("2026-13-01T00:00:00.000Z", "没有这一天"),
        ("2026-00-10T00:00:00.000Z", "没有这一天"),
        ("2026-09-00T00:00:00.000Z", "没有这一天"),
        ("2026-09-25T24:00:00.000Z", "没有这个时刻"),
        ("2026-09-25T07:60:00.000Z", "没有这个时刻"),
        ("2026-09-25T07:04:60.000Z", "没有这个时刻"),
        ("2026-09-25T07:04:05Z", "24 个字符"),
        ("2026-09-25T07:04:05.123+08:00", "24 个字符"),
        ("2026-09-25T07:04:05.123z", "写成"),
        ("2026-09-25 07:04:05.123Z", "写成"),
        ("+026-09-25T07:04:05.123Z", "只能写数字"),
        ("2026-09-2xT07:04:05.123Z", "只能写数字"),
    ] {
        let err = Timestamp::parse(text).unwrap_err();
        assert!(err.to_string().contains(why), "{text}：{err}");
    }
    assert!(Timestamp::parse("2000-02-29T00:00:00.000Z").is_ok());
    assert!(serde_json::from_str::<Timestamp>("1790319845123").is_err());
}

fn offset(minutes: i32) -> UtcOffset {
    UtcOffset::from_minutes(minutes).unwrap()
}

#[test]
fn offsets_are_written_with_utc_and_a_sign() {
    for (minutes, written) in [
        (540, "UTC+09:00"),
        (0, "UTC+00:00"),
        (-300, "UTC-05:00"),
        (330, "UTC+05:30"),
        (-210, "UTC-03:30"),
        (840, "UTC+14:00"),
        (-840, "UTC-14:00"),
    ] {
        assert_eq!(offset(minutes).to_string(), written);
        assert_eq!(offset(minutes).minutes(), minutes);
    }
}

#[test]
fn offsets_beyond_fourteen_hours_are_refused() {
    assert_eq!(UtcOffset::from_minutes(841), None);
    assert_eq!(UtcOffset::from_minutes(-841), None);
}

#[test]
fn the_local_hour_is_the_wall_clock_to_the_hour() {
    for (utc, minutes, local) in [
        ("2026-09-25T07:04:05.140Z", 540, "Fri 2026-09-25 16:00"),
        ("2026-09-25T07:04:05.140Z", 330, "Fri 2026-09-25 12:00"),
        ("2026-12-31T20:30:00.000Z", 540, "Fri 2027-01-01 05:00"),
        ("2026-03-01T02:00:00.000Z", -300, "Sat 2026-02-28 21:00"),
        ("2028-02-29T12:00:00.000Z", 0, "Tue 2028-02-29 12:00"),
        ("1969-12-31T23:00:00.000Z", 0, "Wed 1969-12-31 23:00"),
        ("2026-09-25T00:30:00.000Z", -210, "Thu 2026-09-24 21:00"),
        ("2026-09-25T23:59:59.999Z", 1, "Sat 2026-09-26 00:00"),
    ] {
        let t = Timestamp::parse(utc).unwrap();
        assert_eq!(
            t.local_hour(offset(minutes)),
            local,
            "{utc} 在 {minutes} 分钟的时区"
        );
    }
}

#[test]
fn every_day_of_the_week_has_its_name() {
    let sunday = at("2026-09-20T12:00:00.000Z");
    let names: Vec<String> = (0..7)
        .map(|day| {
            let t = Timestamp::from_unix_millis(sunday + day * MS_PER_DAY).unwrap();
            t.local_hour(offset(0))[..3].to_string()
        })
        .collect();
    assert_eq!(names, ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]);
}

//! `usage.query` 的参数（施工 8-15）：时区的写法、分组的名字、时刻、会话编号，不对的 `bad_params`；不写时区的照核心所在机器
//! 此刻的；只写 `tree` 不写会话的不算。

use serde_json::json;

use super::*;

fn params(value: Value) -> QueryParams {
    serde_json::from_value(value).unwrap()
}

/// 核心所在机器此刻的时区：测试里定成 +08:00。
fn here() -> UtcOffset {
    UtcOffset::from_minutes(480).unwrap()
}

#[test]
fn offsets_are_written_with_a_sign_and_minutes() {
    for (text, minutes) in [
        ("+09:00", 540),
        ("-05:30", -330),
        ("+00:00", 0),
        ("+14:00", 840),
    ] {
        assert_eq!(offset(text), UtcOffset::from_minutes(minutes), "{text}");
    }
    for text in [
        "09:00",
        "+9:00",
        "+09:60",
        "+14:01",
        "+15:00",
        "UTC+09:00",
        "+0900",
        "+09:0x",
        "",
    ] {
        assert_eq!(offset(text), None, "{text}");
    }
}

#[test]
fn the_parameters_are_read_and_checked() {
    let query = read(
        params(json!({
            "from": "2026-10-01T00:00:00.000Z",
            "until": null,
            "group": ["model", "day", "purpose"],
            "session": "01900000-0000-7000-8000-000000000001",
            "tree": true,
            "offset": "+05:30",
            "whatever": 1,
        })),
        here,
    )
    .unwrap();
    assert_eq!(query.group, [Group::Model, Group::Day, Group::Purpose]);
    assert_eq!(query.offset, UtcOffset::from_minutes(330).unwrap());
    assert_eq!(query.until, None);
    assert!(query.from.is_some());
    assert!(matches!(&query.session, Some((_, true))));
    let plain = read(params(json!({"tree": true})), here).unwrap();
    assert_eq!(
        (plain.session, plain.offset, plain.group),
        (None, here(), Vec::new())
    );
    for bad in [
        json!({"group": ["provider"]}),
        json!({"group": ["Model"]}),
        json!({"from": "yesterday"}),
        json!({"session": "abc"}),
        json!({"offset": "Asia/Tokyo"}),
    ] {
        let read = read(params(bad.clone()), here);
        assert!(matches!(read, Err(Refusal::BAD_PARAMS)), "{bad}");
    }
}

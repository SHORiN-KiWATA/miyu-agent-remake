//! 测试共用的几样：造消息、造情形、造此刻、从原文读限流和睡眠。时刻都是写死的数，不碰时钟。

use miyu_kernel::id::ExternalId;
use miyu_kernel::time::{Timestamp, UtcOffset};

use crate::VenueKind;

use super::{Base64, Chain, Clock, Ctx, Inbound, Moderation, Rate, Said, Sleep, Standing, Verdict};

/// 一分钟的毫秒数。
pub(crate) const MINUTE: i64 = 60_000;

/// 一秒的毫秒数。
pub(crate) const SECOND: i64 = 1_000;

/// 测试用的那一天的 UTC 零点：纪元后第 20000 天。
pub(crate) const DAY0: i64 = 20_000 * 24 * 60 * MINUTE;

/// 自 Unix 纪元起 `ms` 毫秒那一刻，断定在内核的范围里。
pub(crate) fn time(ms: i64) -> Timestamp {
    Timestamp::from_unix_millis(ms).expect("在 0000 年到 9999 年里")
}

/// 时刻 `at` 往后 `ms` 毫秒（负的往前）。
pub(crate) fn plus(at: Timestamp, ms: i64) -> Timestamp {
    time(at.unix_millis() + ms)
}

/// 自 Unix 纪元起 `ms` 毫秒那一刻，时区离 UTC 差 `offset` 分钟（东边是正的）。
pub(crate) fn clock(ms: i64, offset: i32) -> Clock {
    Clock {
        now: time(ms),
        offset: UtcOffset::from_minutes(offset).expect("时区在 ±14:00 里"),
    }
}

/// 那一天 UTC `hour:minute` 整，时区离 UTC 差 `offset` 分钟。
pub(crate) fn utc(hour: i64, minute: i64, offset: i32) -> Clock {
    clock(DAY0 + (hour * 60 + minute) * MINUTE, offset)
}

/// 那一天 UTC `hour:minute` 整，时区就是 UTC。
pub(crate) fn at(hour: i64, minute: i64) -> Clock {
    utc(hour, minute, 0)
}

/// 什么都没设的情形：不限流、不睡、没设 `allow`、没被禁言、没有回合、违规关键词是空的。base64 的三个数是测试随手定的。
pub(crate) fn ctx() -> Ctx {
    Ctx {
        rate: None,
        sleep: None,
        allow: None,
        muted: false,
        turns: Vec::new(),
        notices: Vec::new(),
        moderation: Moderation {
            keywords: Vec::new(),
            base64: Base64 {
                min_chars: 8,
                max_chars: 100,
                printable: 900,
            },
        },
    }
}

/// 一条消息：正文是 `hello`，不冲她来。
pub(crate) fn msg(standing: Standing, kind: VenueKind) -> Inbound {
    Inbound {
        kind,
        said: Said {
            sender: ExternalId::parse("qq:10002").expect("合写法"),
            standing,
            addressed: false,
        },
        text: "hello".to_string(),
    }
}

/// 群里别的人发的一条消息。
pub(crate) fn member() -> Inbound {
    msg(Standing::Member, VenueKind::Group)
}

/// 发的人的几种组合：主人在群里、自己人在私聊里、自己人在群里、别的人在私聊里、别的人在群里。
pub(crate) const PEOPLE: [(Standing, VenueKind); 5] = [
    (Standing::Owner, VenueKind::Group),
    (Standing::Trusted, VenueKind::Private),
    (Standing::Trusted, VenueKind::Group),
    (Standing::Member, VenueKind::Private),
    (Standing::Member, VenueKind::Group),
];

/// 从原文读限流，断定写对了。
pub(crate) fn rate(text: &str) -> Option<Rate> {
    Some(Rate::read(text).expect(text))
}

/// 从原文读睡眠，断定写对了。
pub(crate) fn sleep(text: &str) -> Option<Sleep> {
    Some(Sleep::read(text).expect(text))
}

/// 自带的链判一条消息。
pub(crate) fn judge(msg: &Inbound, ctx: &Ctx, clock: Clock) -> Verdict {
    Chain::builtin().judge(msg, ctx, clock)
}

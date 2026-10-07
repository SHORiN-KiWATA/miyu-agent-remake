//! 测试共用的几样：造参数、造消息、造回复、此刻。时刻都是写死的数，不碰时钟。

use miyu_kernel::id::{ExternalId, Seq, VenueId};
use miyu_kernel::time::{Timestamp, UtcOffset};

use crate::{Clock, Flag, Said, Standing};

use super::{Chatty, Conditions, Facts, Reply, Restraint, Window, conditions};

/// 一秒的毫秒数。
pub(crate) const SECOND: i64 = 1_000;

/// 一分钟的毫秒数。
pub(crate) const MINUTE: i64 = 60 * SECOND;

/// 此刻：纪元后第 20000 天的 UTC 12:00。
pub(crate) fn now() -> Clock {
    Clock {
        now: Timestamp::from_unix_millis(20_000 * 24 * 60 * MINUTE + 12 * 60 * MINUTE)
            .expect("在范围里"),
        offset: UtcOffset::UTC,
    }
}

/// 发消息的那个人。
pub(crate) const SENDER: &str = "qq:10002";

/// 另一个人。
pub(crate) const OTHER: &str = "qq:10003";

/// 旧版的默认值（18 第七节），只有抽样是 0：别的测试不让抽样掺进来，抽样的测试自己设。
pub(crate) fn chatty() -> Chatty {
    Chatty {
        probability: 0,
        base: 0.8,
        weights: [0.25, 0.25, 0.15, 0.15, 0.20],
        adjust: 0.2,
        direct: 0.3,
        continuation: Window {
            bonus: 0.1,
            window: 15 * SECOND,
        },
        after_speaking: Window {
            bonus: 0.1,
            window: 30 * SECOND,
        },
        restraint: Restraint {
            on: true,
            half_life: 3 * MINUTE,
            cap: 0.35,
            k: 2.5,
        },
        severity_min: 7,
    }
}

/// 群里别的人发的一条有字的消息，不冲她来，什么都没 @、没引用。
pub(crate) fn facts() -> Facts {
    Facts {
        venue: VenueId::parse("qq:group:123456").expect("合写法"),
        msg: seq(1001),
        said: Said {
            sender: person(SENDER),
            standing: Standing::Member,
            addressed: false,
        },
        mentions_others: false,
        quotes_other: false,
        textless: false,
        media_only: false,
    }
}

/// 平台上的一个人，断定合写法。
pub(crate) fn person(id: &str) -> ExternalId {
    ExternalId::parse(id).expect(id)
}

/// 一个序号，断定不是 0。
pub(crate) fn seq(n: u64) -> Seq {
    Seq::new(n).expect("序号从 1 起")
}

/// 此刻之前 `ago` 毫秒那一刻（负的是以后）。
pub(crate) fn ago(ago: i64) -> Timestamp {
    Timestamp::from_unix_millis(now().now.unix_millis() - ago).expect("在范围里")
}

/// 她在此刻之前 `ago` 毫秒回了 `to` 这几个人一轮；空的是没有明确回谁。
pub(crate) fn reply(ago_ms: i64, to: &[&str]) -> Reply {
    Reply {
        at: ago(ago_ms),
        to: to.iter().map(|id| person(id)).collect(),
    }
}

/// 自带的加值项算一条消息，此刻是 [`now`]。
pub(crate) fn hits(
    facts: &Facts,
    flags: &[Flag],
    replies: &[Reply],
    chatty: &Chatty,
) -> Conditions {
    conditions(facts, flags, replies, now(), chatty)
}

/// 四舍五入到两位小数，比的是百分之几：曲线的值、例子里的分都比到这一位（施工单「风险」第 1 条）。
pub(crate) fn cents(value: f64) -> i64 {
    (value * 100.0).round() as i64
}

/// 四舍五入到千分之几：例子里的分有三位小数。
pub(crate) fn mills(value: f64) -> i64 {
    (value * 1000.0).round() as i64
}

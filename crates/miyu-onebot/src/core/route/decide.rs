//! 判一条群消息（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 5 到 7 条）：过进站链（`Chain::builtin`），放行的算加值项
//! （`conditions`）、定走哪条路（`route`），得出结论；判断写成 `ext.onebot.chat.decided` 的 `body`（「施工时定的」第 76 条）。
//! 纯逻辑：要的都由调的一方从场所规则、投影、这一条填好交进来（`called.rs`）。
//!
//! 这一步只走不用判官的路（施工单「要定的」第 4 条）：主人冲她来的回，没条件的只记下，要问判官的两条路（`Judge`、
//! `ModerationOnly`）结论是「判官还没接」。只有主线，不分派（「施工时定的」第 74 条）。

use miyu_chat::{
    Chain, Chatty, Clock, Conditions, Ctx, Facts, Flag, Inbound, Kind, Outcome, Reply, Route,
    Standing, VenueKind, Verdict, Why, conditions, route,
};
use serde_json::{Value, json};

use super::projection::RATE_LIMITED;

/// 判一条要的。
pub(super) struct Case<'a> {
    /// 这条消息的平台事实：发的人、他是谁、是不是冲她来的都在 `said` 里，进站链也照它。
    pub(super) facts: Facts,
    /// 正文：交给核心的那一份，违规关键词查它。
    pub(super) text: String,
    /// 这个群此刻的情形：规则的限流、睡眠、能不能叫她，投影的回合和提示。
    pub(super) ctx: Ctx,
    /// 她在这个群最近的回复（投影）。
    pub(super) replies: &'a [Reply],
    /// 此刻。
    pub(super) clock: Clock,
    /// 这个群的参数。
    pub(super) chatty: &'a Chatty,
}

/// 结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Conclusion {
    /// 回：开一轮（主人冲她来）。
    Reply,
    /// 只记下。
    Record,
    /// 回一句提示（限流满了冲她来的）：为什么，写进提示的 `reason`。
    Notice(Why),
    /// 要问判官：判官还没接（O-23 下），这一步不回。
    NoJudge,
}

/// 一次判断。
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Decision {
    /// 进站链的判定。
    pub(super) verdict: Verdict,
    /// 放行了的：成立的条件、走的路。
    pub(super) passed: Option<(Conditions, Route)>,
    /// 结论。
    pub(super) conclusion: Conclusion,
}

/// 判一条（「群里怎么叫她」第 5、6 条）：进站链只记下的只记下，提示的回一句；放行的照条件走路：`Record` 只记下，`Commit`
/// 回，`Judge`、`ModerationOnly` 判官还没接。
pub(super) fn decide(case: &Case<'_>) -> Decision {
    let inbound = Inbound {
        kind: VenueKind::Group,
        said: case.facts.said.clone(),
        text: case.text.clone(),
    };
    let verdict = Chain::builtin().judge(&inbound, &case.ctx, case.clock);
    let (passed, conclusion) = match verdict.outcome {
        Outcome::RecordOnly(_) => (None, Conclusion::Record),
        Outcome::Notice(why) => (None, Conclusion::Notice(why)),
        Outcome::Pass => {
            let found = conditions(
                &case.facts,
                &verdict.flags,
                case.replies,
                case.clock,
                case.chatty,
            );
            let way = route(&found, case.facts.said.standing);
            let conclusion = match way {
                Route::Record => Conclusion::Record,
                Route::Commit => Conclusion::Reply,
                Route::Judge | Route::ModerationOnly => Conclusion::NoJudge,
            };
            (Some((found, way)), conclusion)
        }
    };
    Decision {
        verdict,
        passed,
        conclusion,
    }
}

impl Decision {
    /// `ext.onebot.chat.decided` 的 `body`（「群里怎么叫她」第 7 条那张表）：判的是序号 `msgs` 那几条，发的人是 `standing`。
    pub(super) fn body(&self, msgs: &[u64], standing: Standing) -> Value {
        let mut body = json!({
            "msgs": msgs,
            "standing": standing_name(standing),
            "outcome": self.conclusion.name(),
        });
        let (inbound, why) = match self.verdict.outcome {
            Outcome::Pass => ("pass", None),
            Outcome::RecordOnly(why) => ("record_only", Some(why)),
            Outcome::Notice(why) => ("notice", Some(why)),
        };
        body["inbound"] = json!(inbound);
        if let Some(why) = why {
            body["why"] = json!(why_name(why));
        }
        if !self.verdict.flags.is_empty() {
            let flags: Vec<&str> = self
                .verdict
                .flags
                .iter()
                .map(|flag| flag_name(*flag))
                .collect();
            body["flags"] = json!(flags);
        }
        if let Some((found, way)) = &self.passed {
            let hits: Vec<Value> = found
                .hits
                .iter()
                .map(|hit| json!({"kind": kind_name(hit.kind), "bonus": hit.bonus}))
                .collect();
            body["conditions"] = json!(hits);
            body["route"] = json!(route_name(*way));
        }
        body
    }
}

impl Conclusion {
    /// `body` 里 `outcome` 的写法；运行日志也照它。
    pub(super) fn name(self) -> &'static str {
        match self {
            Conclusion::Reply => "reply",
            Conclusion::Record => "record",
            Conclusion::Notice(_) => "notice",
            Conclusion::NoJudge => "no_judge",
        }
    }
}

/// 没放行的原因的写法：`body` 的 `why`，提示的 `reason`。
pub(super) fn why_name(why: Why) -> &'static str {
    match why {
        Why::Asleep => "asleep",
        Why::Muted => "muted",
        Why::NotAllowed => "not_allowed",
        Why::RateLimited => RATE_LIMITED,
    }
}

/// 发的人是谁的写法。
fn standing_name(standing: Standing) -> &'static str {
    match standing {
        Standing::Owner => "owner",
        Standing::Trusted => "trusted",
        Standing::Member => "member",
    }
}

/// 旗的写法。
fn flag_name(flag: Flag) -> &'static str {
    match flag {
        Flag::Moderation => "moderation",
    }
}

/// 条件的种类的写法。
fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Direct => "direct",
        Kind::Continuation => "continuation",
        Kind::AfterSpeaking => "after_speaking",
        Kind::Probability => "probability",
        Kind::Moderation => "moderation",
    }
}

/// 走的路的写法。
fn route_name(way: Route) -> &'static str {
    match way {
        Route::Record => "record",
        Route::Commit => "commit",
        Route::ModerationOnly => "moderation_only",
        Route::Judge => "judge",
    }
}

#[cfg(test)]
mod tests;

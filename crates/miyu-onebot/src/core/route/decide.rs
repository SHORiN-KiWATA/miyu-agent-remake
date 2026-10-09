//! 判一条群消息（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 5、6、10、11、14 条）：过进站链（`Chain::builtin`），放行的
//! 算加值项（`conditions`），照线路规程留下算数的条件（`discipline`，O-23 下）、看顶替（`supersede`，O-23 下）、定走哪条路，
//! 得出结论。纯逻辑：要的都由调的一方从场所规则、投影、桥内存里在判的、这一条填好交进来（`called.rs`）。判断写成
//! `ext.onebot.chat.decided` 的 `body` 在 `body.rs`。
//!
//! 要问判官的结论是 [`Conclusion::Judge`]：判官回来、算完分才知道回不回（`judged.rs`）。额度满了的这段时间 `chatty` 不抽样、
//! 不问判官（第 14 条），要问判官的只记下。只有主线，不分派（「施工时定的」第 74 条）。

use miyu_chat::{
    Chain, Chatty, Clock, Conditions, Ctx, Facts, Inbound, Mode, Outcome, Pending, Reply, Route,
    Supersede, VenueKind, Verdict, Why, conditions, rate_full, supersede,
};

use super::discipline::Discipline;

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
    /// 这个群的线路规程（O-23 下）。
    pub(super) discipline: Discipline,
    /// 这个群还没回完的（O-23 下）：投影里判过要回、她还没回完的，桥内存里在判的。
    pub(super) pendings: &'a [Pending],
    /// 顶替窗口，毫秒（这个群的 `Params::supersede_window`）。
    pub(super) window: i64,
}

/// 结论。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Conclusion {
    /// 回：开一轮。
    Reply,
    /// 只记下。
    Record,
    /// 回一句提示（限流满了冲她来的）：为什么，写进提示的 `reason`。
    Notice(Why),
    /// 问判官：打分，或者只查违规。回不回等判官回来、算完分。
    Judge(Mode),
}

/// 一次判断。
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Decision {
    /// 判的是哪几条：序号，照先后。顶替重判的是那几条，别的只有这一条。
    pub(super) msgs: Vec<u64>,
    /// 进站链的判定。
    pub(super) verdict: Verdict,
    /// 放行了的：线路规程、条件、顶替、走的路。
    pub(super) passed: Option<Passed>,
    /// 结论。
    pub(super) conclusion: Conclusion,
}

/// 放行了的一条怎么走的。
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Passed {
    /// 线路规程。
    pub(super) discipline: Discipline,
    /// 算数的条件：顶替了的是合起来的。
    pub(super) conditions: Conditions,
    /// 顶替。
    pub(super) supersede: Supersede,
    /// 走的路：接过去的是 `Commit`。
    pub(super) route: Route,
    /// 要问判官、额度满了没问（第 14 条）。
    pub(super) rate_full: bool,
}

/// 判一条（「群里怎么叫她」第 5、6、10、11、14 条）：进站链只记下的只记下，提示的回一句；放行的照线路规程留下条件、看顶替：
/// 接过去的回，别的照线路规程走路：`Record` 只记下，`Commit` 回，`Judge`、`ModerationOnly` 问判官（额度满了的只记下）。
pub(super) fn decide(case: &Case<'_>) -> Decision {
    let own = case.facts.msg.get();
    let inbound = Inbound {
        kind: VenueKind::Group,
        said: case.facts.said.clone(),
        text: case.text.clone(),
    };
    let verdict = Chain::builtin().judge(&inbound, &case.ctx, case.clock);
    let (passed, conclusion, msgs) = match verdict.outcome {
        Outcome::RecordOnly(_) => (None, Conclusion::Record, vec![own]),
        Outcome::Notice(why) => (None, Conclusion::Notice(why), vec![own]),
        Outcome::Pass => {
            let (passed, msgs) = pass(case, &verdict);
            let conclusion = match passed.route {
                Route::Commit => Conclusion::Reply,
                Route::Record => Conclusion::Record,
                _ if passed.rate_full => Conclusion::Record,
                Route::Judge => Conclusion::Judge(Mode::Reply),
                Route::ModerationOnly => Conclusion::Judge(Mode::ModerationOnly),
            };
            (Some(passed), conclusion, msgs)
        }
    };
    Decision {
        msgs,
        verdict,
        passed,
        conclusion,
    }
}

/// 放行了的一条：算条件、照线路规程留下、看顶替、走路；交回怎么走的和判的是哪几条。
fn pass(case: &Case<'_>, verdict: &Verdict) -> (Passed, Vec<u64>) {
    let discipline = case.discipline;
    let found = conditions(
        &case.facts,
        &verdict.flags,
        case.replies,
        case.clock,
        case.chatty,
    );
    // 额度满了的这段时间不抽样、不问判官（18 第六节）：抽样只有 `chatty` 有，问判官哪种规程都照它。
    let full = rate_full(&case.ctx, case.clock);
    let own = discipline.keep(found, full);
    let superseded = match discipline.supersedes() {
        true => supersede(&case.facts, &own, case.pendings, case.clock, case.window),
        false => Supersede::None,
    };
    let standing = case.facts.said.standing;
    let media_only = case.facts.media_only;
    let (conditions, route, msgs) = match &superseded {
        Supersede::None => {
            let route = discipline.route(&own, standing, media_only);
            (own, route, vec![case.facts.msg.get()])
        }
        Supersede::Inherit { conditions, .. } => (
            conditions.clone(),
            Route::Commit,
            vec![case.facts.msg.get()],
        ),
        Supersede::Rejudge {
            msgs, conditions, ..
        } => {
            let route = discipline.route(conditions, standing, media_only);
            let msgs = msgs.iter().map(|msg| msg.get()).collect();
            (conditions.clone(), route, msgs)
        }
    };
    let rate_full = full && matches!(route, Route::Judge | Route::ModerationOnly);
    let passed = Passed {
        discipline,
        conditions,
        supersede: superseded,
        route,
        rate_full,
    };
    (passed, msgs)
}

#[cfg(test)]
mod follow_tests;
#[cfg(test)]
mod tests;

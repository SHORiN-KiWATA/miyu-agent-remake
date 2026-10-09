//! 算分（`docs/blueprint/chat.md` 第三条「怎么走」第 9、11 条，18 第七节「算分」）：拿判官的回答，算出 `raw`、调整、加分、
//! 门槛修正、门槛、总分和结论。每一项都交出去，记进日志（`ext.onebot.chat.decided` 的形状随核心的场所事件）。
//!
//! 数都是 `f64`，比较时不另留余量：同一台机器上回放字字相同，跨平台末位的差别只在正好压线时可能翻，接受。

use crate::Clock;

use super::{Chatty, Conditions, LiftCtx, Reply, lifts};

/// 判官的回答：[`read`](crate::read) 从判官回的字里读出来（`chat.md` 第六条「怎么走」第 4 条，施工 O-11）。
#[derive(Debug, Clone, PartialEq)]
pub struct Judgement {
    /// 五维的分，各 0 到 10，照相关、意愿、社交、时机、连贯的先后，和 [`Chatty`] 的五维权重（出厂文件的
    /// `chatty.relevance` 到 `chatty.continuity`）一一对上。超过 10 的当 10，低于 0 的当 0。
    pub scores: [f64; 5],
    /// 判官说该不该回：照它加减 [`Chatty`] 的 `adjust`。
    pub should_reply: bool,
    /// 判官说这句是不是在跟她说话：是的，冷静不抬。
    pub to_bot: bool,
    /// 违规的严重程度，0 到 10；没查是 `None`。不低于 [`Chatty`] 的违规门槛 `severity_min` 的，不管分数都回。
    pub severity: Option<u8>,
    /// 判官的一句理由，最多 500 个字符。只进日志（`ext.onebot.chat.decided`），不进她的上下文，算分不看它（18 第七节「两边
    /// 各看各的」）。
    pub reason: String,
}

/// 算分的每一项和结论（「怎么走」第 9 条）。只查违规的不打分：数都是 0，`reply` 只看违规。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Score {
    /// 五维加权：`Σ 权重ᵢ × 分ᵢ ÷ (10 × Σ 权重ᵢ)`，0 到 1；权重加起来是 0 的是 0。
    pub raw: f64,
    /// `should_reply` 的调整，带着正负号：该回是 `+adjust`，不该回是 `−adjust`。
    pub adjust: f64,
    /// 成立了的条件的加分之和。
    pub bonus: f64,
    /// 门槛修正之和：现在只有冷静。
    pub lift: f64,
    /// 门槛：`max(0, base + lift)`。
    pub threshold: f64,
    /// 总分：`max(0, raw + adjust + bonus)`。
    pub total: f64,
    /// 回不回：总分不低于门槛，或者违规的严重程度不低于门槛。
    pub reply: bool,
}

/// 算分、跟门槛比（「怎么走」第 9、10 条）。`conditions` 只有违规旗的（走 [`Route::ModerationOnly`](super::Route) 的）
/// 不打分，`reply` 只看 `severity`。`replies` 是她在这个场所最近的回复，冷静照它算；先后不要紧。
pub fn score(
    judgement: &Judgement,
    conditions: &Conditions,
    replies: &[Reply],
    clock: Clock,
    chatty: &Chatty,
) -> Score {
    let violated = judgement
        .severity
        .is_some_and(|severity| severity >= chatty.severity_min);
    if conditions.only_moderation() {
        return Score {
            raw: 0.0,
            adjust: 0.0,
            bonus: 0.0,
            lift: 0.0,
            threshold: 0.0,
            total: 0.0,
            reply: violated,
        };
    }
    let raw = raw(&judgement.scores, &chatty.weights);
    let adjust = match judgement.should_reply {
        true => chatty.adjust,
        false => -chatty.adjust,
    };
    let bonus = conditions.hits.iter().map(|hit| hit.bonus).sum::<f64>();
    let total = (raw + adjust + bonus).max(0.0);
    let ctx = LiftCtx {
        judgement,
        conditions,
        replies,
        clock,
        chatty,
    };
    let lift = lifts().iter().map(|lift| lift.lift(&ctx)).sum::<f64>();
    let threshold = (chatty.base + lift).max(0.0);
    Score {
        raw,
        adjust,
        bonus,
        lift,
        threshold,
        total,
        reply: total >= threshold || violated,
    }
}

/// 五维加权，0 到 1。权重加起来是 0 的（全是 0）是 0，不除以 0。
fn raw(scores: &[f64; 5], weights: &[f64; 5]) -> f64 {
    let sum = weights.iter().sum::<f64>();
    if sum == 0.0 {
        return 0.0;
    }
    let weighted = weights
        .iter()
        .zip(scores)
        .map(|(weight, score)| weight * score.clamp(0.0, 10.0))
        .sum::<f64>();
    weighted / (10.0 * sum)
}

#[cfg(test)]
mod tests;

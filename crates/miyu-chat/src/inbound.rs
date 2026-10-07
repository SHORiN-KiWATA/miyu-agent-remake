//! 进站链与限流的判定（`docs/blueprint/chat.md` 第二条，`docs/designs/18-通讯平台.md` 第六节、第十六节，施工 O-5）。
//!
//! 进来的每条消息先过进站链，像 netfilter 的 INPUT 链：一条条规则（[`InboundRule`]）照顺序过，每条给出往下走、插旗往下走、
//! 或者停下给结果（[`Step`]）。自带五条（[`Chain::builtin`]）：睡眠、她被禁言、谁能叫她、违规关键词、限流，每条一个文件。
//! 核心自己开的回合不过链，先问回合闸（[`gate()`]）。
//!
//! 纯逻辑：此刻和时区差（[`Clock`]）、她被禁言没有、最近开过哪些回合、限流提示过的时刻，都由外面交进来（[`Ctx`]），
//! 这里不碰时钟和时区库（施工时定的第 6 条）。

mod allow;
mod base64;
mod gate;
mod moderation;
mod muted;
mod rate;
mod sleep;

pub use gate::{Gate, gate};
pub use moderation::{Base64, Moderation};
pub use rate::{Rate, rate_full};
pub use sleep::Sleep;

use crate::VenueKind;

/// 发消息的人是谁（`18-通讯平台.md` 第三节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// 主人：核心照 `external.bindings` 认出来的本机账号本人。睡眠、谁能叫她都豁免，不查违规关键词，不计限流。
    Owner,
    /// 自己人：私聊里豁免睡眠和谁能叫她，在哪都不计限流（施工时定的第 2 条）。
    Trusted,
    /// 别的人。
    Member,
}

/// 进来的一条消息，桥照驱动报上来的填好交进来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inbound {
    /// 场所是群还是私聊：自己人只在私聊里豁免睡眠和谁能叫她。
    pub kind: VenueKind,
    /// 发的人，带平台前缀的编号，例如 `qq:10002`。自带的五条不看它，留给往链里加的规则。
    pub sender: String,
    /// 发的人是谁。
    pub standing: Standing,
    /// 正文：违规关键词查它。
    pub text: String,
    /// 是不是冲她来的：@ 她、回复她、叫到名字或触发词，由外面算好（施工单「不做什么」第 1 条）。限流满了只给冲她来的
    /// 回一句。
    pub addressed: bool,
}

/// 此刻，外面交进来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    /// 自 Unix 纪元起的毫秒，可以是负的。
    pub now: i64,
    /// 场所会话的时区离 UTC 差几分钟，东边是正的（北京 `480`，纽约冬天 `-300`）。睡眠照它换成当地时间。
    pub offset: i32,
}

/// 这个场所此刻的情形，由外面从场所规则和场所会话的日志投影出来交进来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ctx {
    /// 限流：没设或者 `"0"`（不限）是 `None`。用 [`Rate::read`] 从原文读。
    pub rate: Option<Rate>,
    /// 睡眠：没设或者 `"off"` 是 `None`。用 [`Sleep::read`] 从原文读。
    pub sleep: Option<Sleep>,
    /// 这个场所能不能叫她（场所规则的 `allow`）：没设是 `None`，当 `true`。
    pub allow: Option<bool>,
    /// 她在这个场所被禁言了没有。
    pub muted: bool,
    /// 最近开过的回合的开始时刻（毫秒），不算主人、自己人开的：外面交进来时就去掉了。先后不要紧；只看窗口里的，交多了
    /// 不要紧。
    pub turns: Vec<i64>,
    /// 限流提示过的时刻（毫秒，`ext.venues.rate_noticed`）。先后不要紧。
    pub notices: Vec<i64>,
    /// 违规关键词的参数。
    pub moderation: Moderation,
}

/// 进站链的结果：这条消息怎么办（`18-通讯平台.md` 第六节那张表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// 放行：交给这个场所的线路规程。
    Pass,
    /// 只记下：记成事件，不往下走。
    RecordOnly(Why),
    /// 拒绝并回一句：记成事件，回一句说明（话随出站那一步）。
    Notice(Why),
}

/// 没放行的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// 睡眠时间里。
    Asleep,
    /// 她被禁言了。
    Muted,
    /// 这个场所不让叫她（`allow = false`）。
    NotAllowed,
    /// 限流的额度满了。
    RateLimited,
}

/// 插在消息上的旗：不拦，交给线路规程。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flag {
    /// 违规旗：命中违规关键词，线路规程让判官认真查一眼（`18-通讯平台.md` 第七节）。
    Moderation,
}

/// 一条规则给出的一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// 这条规则不管，往下走。
    Continue,
    /// 插一面旗，往下走。
    Flag(Flag),
    /// 停下，结果就是它；后面的规则不看。
    Stop(Outcome),
}

/// 进站链的判定：结果和一路插的旗。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// 结果。
    pub outcome: Outcome,
    /// 一路插的旗，照规则的先后；停下之前插的也在。
    pub flags: Vec<Flag>,
}

/// 进站链的插槽：一条规则。扩展往链里加规则也照它写（`18-通讯平台.md` 第十四节）。
///
/// 规则只看交进来的，不碰 I/O、时钟：同样的消息、情形、此刻，给出同样的一步。
pub trait InboundRule {
    /// 规则的名字，例如 `sleep`：说清是哪一条停下的、插的旗。
    fn name(&self) -> &str;

    /// 看一条消息，给出一步。
    fn judge(&self, msg: &Inbound, ctx: &Ctx, clock: Clock) -> Step;
}

/// 进站链：一串规则，照顺序过。
pub struct Chain {
    /// 规则，照先后。
    rules: Vec<Box<dyn InboundRule>>,
}

impl Chain {
    /// 自带的五条，照这个顺序：睡眠、她被禁言、谁能叫她、违规关键词、限流（「怎么走」第 1 条）。
    pub fn builtin() -> Self {
        Self {
            rules: vec![
                Box::new(sleep::Rule),
                Box::new(muted::Rule),
                Box::new(allow::Rule),
                Box::new(moderation::Rule),
                Box::new(rate::Rule),
            ],
        }
    }

    /// 一条消息过链：哪一条给出 [`Step::Stop`] 就停，后面的不看；插旗的接着往下走，旗都记下。都过了是 [`Outcome::Pass`]。
    pub fn judge(&self, msg: &Inbound, ctx: &Ctx, clock: Clock) -> Verdict {
        let mut flags = Vec::new();
        for rule in &self.rules {
            match rule.judge(msg, ctx, clock) {
                Step::Continue => {}
                Step::Flag(flag) => flags.push(flag),
                Step::Stop(outcome) => return Verdict { outcome, flags },
            }
        }
        Verdict {
            outcome: Outcome::Pass,
            flags,
        }
    }
}

impl Inbound {
    /// 睡眠、谁能叫她都豁免的人：主人在哪都豁免，自己人只在私聊里（施工时定的第 2 条）。
    fn excused(&self) -> bool {
        match self.standing {
            Standing::Owner => true,
            Standing::Trusted => self.kind == VenueKind::Private,
            Standing::Member => false,
        }
    }
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

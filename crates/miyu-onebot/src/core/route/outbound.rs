//! 出站（施工 O-25 上，`onebot.md` 第一条「群里怎么叫她」第 9 条、「怎么走」第 10 条）：她的一条话过群聊内核的出站链
//! （`chat.md` 第五条：清理、去重、引用和 @），过了的照 `plain` 转成纯文本、`split` 拆段，交出几段和第一段带的引用、@。纯逻辑：
//! 此刻、参数、这一轮发出去的都由调的一方交进来。
//!
//! - 群里的情形照投影（`projection` 的 [`Speaking`]）：本来想要引用她回的那一条、@ 发它的人；那之后别人说了几条、过了多久、
//!   群里最后一条是不是她的；这一轮入队了的（O-25 中）。
//! - 私聊的两样都是假；这一轮发出去的照桥自己入队了的那几段（[`Spoken`]，入队记成了就记上，内存里一个会话一份，换回合就清，
//!   桥重启就丢）：私聊的订阅不补从前的，照日志算不了（「施工时定的」第 113 条，O-25 中改成入队时记）。

use miyu_chat::{
    Out, OutChain, OutCtx, OutWhy, Outbound, Outgoing, Sent, Since, Target, plain, split,
};
use miyu_kernel::time::Timestamp;

use super::projection::{Aim, Speaking};
use crate::onebot::Lead;

/// 过了链、要发的一句。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Passed {
    /// 照先后的几段：纯文本、拆好了的，没有空的。
    pub(super) pieces: Vec<String>,
    /// 第一段前面带的引用和 @。
    pub(super) lead: Lead,
}

/// 她的一句 `text` 过出站链，情形是 `ctx`：丢了的交回为什么；过了的照 `plain`、`split`（一段最多 `split_chars` 个字符，`0`
/// 不拆）拆段，实际要带的引用、@ 照她回的那一条 `aim` 填（第 9 条第 1 到 3 项）。
pub(super) fn pass(
    text: &str,
    ctx: &OutCtx,
    aim: Option<&Aim>,
    split_chars: usize,
) -> Result<Passed, OutWhy> {
    let outgoing = Outgoing {
        text: text.to_string(),
        images: Vec::new(),
    };
    let (outgoing, target) = match OutChain::builtin().judge(outgoing, ctx) {
        Out::Send { outgoing, target } => (outgoing, target),
        Out::Drop(why) => return Err(why),
    };
    let lead = Lead {
        reply: aim.filter(|_| target.quote).and_then(|aim| aim.msg.clone()),
        at: aim.filter(|_| target.mention).and_then(number),
    };
    Ok(Passed {
        pieces: split(&plain(&outgoing.text), split_chars),
        lead,
    })
}

/// 群里的情形（第 9 条第 1 项）：照投影交出的 `speaking`，此刻是 `now`，参数是这个群此刻的 `outbound`。本来想要的引用、@ 要
/// 她回的那一条有平台编号、发的人解得出号。
pub(super) fn group_ctx(speaking: &Speaking, now: Timestamp, outbound: Outbound) -> OutCtx {
    let aim = speaking.aim.as_ref();
    OutCtx {
        sent: sent(speaking.sent.clone()),
        target: Target {
            quote: aim.is_some_and(|aim| aim.msg.is_some()),
            mention: aim.and_then(number).is_some(),
        },
        since: Since {
            others: speaking.others,
            elapsed: aim.map_or(0, |aim| now.unix_millis() - aim.at.unix_millis()),
            last_is_own: speaking.last_is_own,
        },
        outbound,
    }
}

/// 私聊的情形（「怎么走」第 10 条）：引用、@ 两样都是假，这一轮发出去的是 `texts`，参数是这个私聊此刻的 `outbound`。
pub(super) fn private_ctx(texts: Vec<String>, outbound: Outbound) -> OutCtx {
    OutCtx {
        sent: sent(texts),
        target: Target::default(),
        since: Since {
            others: 0,
            elapsed: 0,
            last_is_own: false,
        },
        outbound,
    }
}

/// 丢了的原因写成什么：运行日志的 `why`（第 9 条第 2 项）。
pub(super) fn why_name(why: OutWhy) -> &'static str {
    match why {
        OutWhy::Leaked => "leaked",
        OutWhy::Blank => "blank",
        OutWhy::Aside => "aside",
        OutWhy::Repeated => "repeated",
    }
}

/// 私聊里桥这一轮自己入队了的那几段（见模块开头）。
#[derive(Debug, Default)]
pub(super) struct Spoken {
    /// 回合编号：换了回合就清。
    turn: u64,
    /// 这一轮入队了的正文（拆好的每一段），照先后。
    texts: Vec<String>,
}

impl Spoken {
    /// 回合编号是 `turn` 的那一轮已经发出去的；别的回合的不算。
    pub(super) fn of(&self, turn: u64) -> Vec<String> {
        if self.turn == turn {
            self.texts.clone()
        } else {
            Vec::new()
        }
    }

    /// 回合编号是 `turn` 的那一轮入队了一段 `text`：换了回合的从这一轮重新记。
    pub(super) fn add(&mut self, turn: u64, text: String) {
        if self.turn != turn {
            self.turn = turn;
            self.texts.clear();
        }
        self.texts.push(text);
    }
}

/// 这一回合已经发出去的正文 `texts`：她的图随后面的步子，没有图。
fn sent(texts: Vec<String>) -> Sent {
    Sent {
        texts,
        images: Vec::new(),
    }
}

/// 她回的那一条的发的人的号：`<平台>:<号>` 的后一段（@ 段的 `qq`）；解不出的是空的。
fn number(aim: &Aim) -> Option<String> {
    miyu_chat::parse_person(&aim.sender).map(|(_, number)| number.to_string())
}

#[cfg(test)]
mod tests;

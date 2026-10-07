//! 出站链与纯文本（`docs/blueprint/chat.md` 第五条，`docs/designs/18-通讯平台.md` 第十节、Q8、Q14，施工 O-10）。
//!
//! 她要说出去的一切先过出站链，像 netfilter 的 OUTPUT 链：一条条规则（[`OutboundRule`]）照顺序过，每条可以改写这一条
//! 往下走，或者整条丢掉（[`OutStep`]）。自带三条（[`OutChain::builtin`]）：清理、去重、引用和 @，每条一个文件。
//!
//! 过了链的照出站形态发；这一步只有纯文本一种形态：[`plain()`] 把 Markdown 转成纯文本，[`split()`] 把太长的按段拆开。
//!
//! 纯逻辑：这一回合已经发出去的（[`Sent`]）、她回的那条之后群里的动静（[`Since`]）、参数（[`Outbound`]），都由外面交进来
//! （[`OutCtx`]）；出站队列（禁言暂停、过期作废、回执撤回、退信）随桥（「怎么走」第 7 条）。

use miyu_kernel::id::ContentHash;

mod clean;
mod dedupe;
mod plain;
mod split;
mod target;

pub use plain::plain;
pub use split::split;

/// 要发的一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outgoing {
    /// 正文。清理时整条是空白的、去重时正文重复而带图的，正文变成空的。
    pub text: String,
    /// 几张图，每张是内容的哈希（内核的 `ContentHash`，blob 的编号）；去重看它。先后就是发的先后。
    pub images: Vec<ContentHash>,
}

/// 这一回合已经发出去的：去重只看这一回合（施工时定的第 1 条），外面换回合时清空。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sent {
    /// 发出去的正文，原文；归一化在这里做。
    pub texts: Vec<String>,
    /// 发出去的图的哈希。
    pub images: Vec<ContentHash>,
}

/// 引用、@ 那个人：这一条要不要。
///
/// 交进来的是「本来想要」：外面照这一轮的 `reply-to` 定好（施工时定的第 3 条），私聊两样都是假；引用和 @ 那条规则决定
/// 实际带不带。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Target {
    /// 引用她回的那条消息。
    pub quote: bool,
    /// @ 发那条消息的人。
    pub mention: bool,
}

/// 她回的那条消息之后，群里的动静：外面从场所会话的日志数出来交进来。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Since {
    /// 来了几条别人的消息：不算那个人自己补的，也不算她自己的。
    pub others: u64,
    /// 从那条消息进来到此刻过了多少毫秒；负的当没过。
    pub elapsed: i64,
    /// 群里最后一条是不是她自己的：她连着说话时靠引用分清在回谁。
    pub last_is_own: bool,
}

/// 引用和 @ 的两个参数。出厂的数（4 条、15 秒）随桥放进出厂数据，代码里不写死（施工时定的第 4 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outbound {
    /// 隔几条别人的消息才引用；`0` 是总引用。
    pub quote_after: u64,
    /// 隔多少毫秒才 @。
    pub mention_after: i64,
    /// 去重：这一条的两字组至少这么多个才比相似度，太短的句子换几个字就差很多，比了只会误杀（出厂 16，旧版实测）。
    pub min_bigrams: usize,
    /// 去重：两字组的 Jaccard 相似度不低于这个百分比算重复（出厂 66，旧版实测）；用整数比，不碰小数的舍入。
    pub similar: u8,
}

/// 出站链看的情形，由外面交进来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutCtx {
    /// 这一回合已经发出去的。
    pub sent: Sent,
    /// 本来想不想引用、@。
    pub target: Target,
    /// 她回的那条消息之后群里的动静。
    pub since: Since,
    /// 参数。
    pub outbound: Outbound,
}

/// 整条丢掉的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutWhy {
    /// 整条被漏进正文的工具调用占满了。
    Leaked,
    /// 只有空白和不可见字符，没有图。
    Blank,
    /// 整条是一对中文括号括起来的旁白，没有图。
    Aside,
    /// 这一回合已经发过了：正文重复没有图，或者图都重复、正文也没了。
    Repeated,
}

/// 出站链的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Out {
    /// 发：改写过的这一条，实际带不带引用和 @。
    Send {
        /// 要发的这一条。
        outgoing: Outgoing,
        /// 实际带不带引用和 @。
        target: Target,
    },
    /// 整条丢掉。
    Drop(OutWhy),
}

/// 一条规则给出的一步。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutStep {
    /// 往下走：这一条和引用、@ 可以改写过。
    Continue {
        /// 往下交的这一条。
        outgoing: Outgoing,
        /// 往下交的引用和 @。
        target: Target,
    },
    /// 整条丢掉，后面的规则不看。
    Drop(OutWhy),
}

/// 出站链的插槽：一条规则。扩展往链里加规则也照它写（`18-通讯平台.md` 第十四节）。
///
/// 规则只看交进来的，不碰 I/O、时钟：同样的一条、同样的情形，给出同样的一步。
pub trait OutboundRule {
    /// 规则的名字，例如 `clean`：说清是哪一条丢的。
    fn name(&self) -> &str;

    /// 看一条，给出一步。`target` 是前面的规则交下来的引用和 @，第一条拿到的是 [`OutCtx::target`]。
    fn judge(&self, outgoing: Outgoing, target: Target, ctx: &OutCtx) -> OutStep;
}

/// 出站链：一串规则，照顺序过。
pub struct OutChain {
    /// 规则，照先后。
    rules: Vec<Box<dyn OutboundRule>>,
}

impl OutChain {
    /// 自带的三条，照这个顺序：清理、去重、引用和 @（「怎么走」第 1 条）。
    pub fn builtin() -> Self {
        Self {
            rules: vec![
                Box::new(clean::Rule),
                Box::new(dedupe::Rule),
                Box::new(target::Rule),
            ],
        }
    }

    /// 一条过链：哪一条丢了就停，后面的不看；都过了是 [`Out::Send`]，带着一路改写过的这一条和引用、@。
    pub fn judge(&self, outgoing: Outgoing, ctx: &OutCtx) -> Out {
        let mut outgoing = outgoing;
        let mut target = ctx.target;
        for rule in &self.rules {
            match rule.judge(outgoing, target, ctx) {
                OutStep::Continue {
                    outgoing: next,
                    target: wanted,
                } => {
                    outgoing = next;
                    target = wanted;
                }
                OutStep::Drop(why) => return Out::Drop(why),
            }
        }
        Out::Send { outgoing, target }
    }
}

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

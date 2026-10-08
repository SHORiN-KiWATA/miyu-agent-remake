//! 清理（`docs/blueprint/chat.md` 第五条「怎么走」第 2 条）：去掉漏进正文的工具调用；整条是空的、整条是括号旁白的丢掉。
//! 两份名单（漏进来的标记、不可见字符）是数据，从 [`Outbound`] 拿，出厂的照旧版（施工时定的第 2、10 条，第八条）。
//!
//! 漏进来的样子是旧版线上取的证：模型复读退化时把 `<tool_call><function=…>` 当正文吐出来，中转站的解析器不认，原样落进
//! 正文发到群里；流式截断的只剩开头，没有收尾。

use super::{OutCtx, OutStep, OutWhy, Outbound, OutboundRule, Outgoing, Target};

/// 清理这条规则。
pub(super) struct Rule;

impl OutboundRule for Rule {
    fn judge(&self, mut outgoing: Outgoing, target: Target, ctx: &OutCtx) -> OutStep {
        let leaked = strip_leaks(&mut outgoing.text, &ctx.outbound);
        let pictured = !outgoing.images.is_empty();
        if blank(&outgoing.text, &ctx.outbound.invisible) {
            match (pictured, leaked) {
                (false, true) => return OutStep::Drop(OutWhy::Leaked),
                (false, false) => return OutStep::Drop(OutWhy::Blank),
                // 有图的照发图，空白的正文不发成一个空气泡。
                (true, _) => outgoing.text.clear(),
            }
        }
        if !pictured && aside(outgoing.text.trim()) {
            return OutStep::Drop(OutWhy::Aside);
        }
        OutStep::Continue { outgoing, target }
    }
}

/// 一段一段去掉漏进来的工具调用，直到没有：开头、收尾照位置一一对上（[`Outbound`] 的 `leak_open`、`leak_close`）；每次去
/// 最早开头的一段，从开头到它后面的第一个收尾，找不到收尾的去到末尾。去掉过东西的是 `true`。
///
/// 收尾从开头的后面找起：收尾可能是开头的一段（开头 ` ```tool `、收尾 ` ``` `），从开头处找只去掉半个开头（施工时定的
/// 第 11 条）。开头都不空（声明守着），每一轮至少去掉一个开头，停得下来。
fn strip_leaks(text: &mut String, outbound: &Outbound) -> bool {
    let mut stripped = false;
    while let Some((start, open, close)) = outbound
        .leak_open
        .iter()
        .zip(&outbound.leak_close)
        .filter_map(|(open, close)| text.find(open.as_str()).map(|at| (at, open, close)))
        .min_by_key(|(at, ..)| *at)
    {
        let after = start + open.len();
        let end = text
            .get(after..)
            .and_then(|rest| rest.find(close.as_str()))
            .map_or(text.len(), |at| after + at + close.len());
        text.replace_range(start..end, "");
        stripped = true;
    }
    stripped
}

/// 看起来是空的：只有空白和 `invisible` 里的字。模型「什么都不想说」时常吐一个零宽空格，`trim` 不认它，发出去是一个空
/// 气泡。只拿来判空，不拿来改正文：零宽连接符夹在表情里是有意义的。
fn blank(text: &str, invisible: &[char]) -> bool {
    text.chars()
        .all(|c| c.is_whitespace() || invisible.contains(&c))
}

/// 整条是一对中文括号括起来的旁白：以 `（` 开头、和它配对的 `）` 在末尾，里面可以再套括号，括号外没有别的字。`text`
/// 交进来前已经去掉首尾的空白。
fn aside(text: &str) -> bool {
    let mut depth = 0_u32;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '（' => depth += 1,
            '）' if depth == 0 => return false,
            '）' => {
                depth -= 1;
                // 最外面那一对关上了，后面还有字：括号外有字。
                if depth == 0 && chars.peek().is_some() {
                    return false;
                }
            }
            _ if depth == 0 => return false,
            _ => {}
        }
    }
    // 空的不算；没关上的不算。
    !text.is_empty() && depth == 0
}

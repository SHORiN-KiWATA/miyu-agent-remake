//! 清理（`docs/blueprint/chat.md` 第五条「怎么走」第 2 条）：去掉漏进正文的工具调用；整条是空的、整条是括号旁白的丢掉。
//! 数和字的范围照旧版（施工时定的第 2 条）。
//!
//! 漏进来的样子是旧版线上取的证：模型复读退化时把 `<tool_call><function=…>` 当正文吐出来，中转站的解析器不认，原样落进
//! 正文发到群里；流式截断的只剩开头，没有收尾。

use super::{OutCtx, OutStep, OutWhy, OutboundRule, Outgoing, Target};

/// 漏进正文的工具调用：开头和收尾。
const LEAKS: [(&str, &str); 2] = [
    ("<tool_call>", "</tool_call>"),
    ("<function=", "</function>"),
];

/// 清理这条规则。
pub(super) struct Rule;

impl OutboundRule for Rule {
    fn name(&self) -> &str {
        "clean"
    }

    fn judge(&self, mut outgoing: Outgoing, target: Target, _ctx: &OutCtx) -> OutStep {
        let leaked = strip_leaks(&mut outgoing.text);
        let pictured = !outgoing.images.is_empty();
        if blank(&outgoing.text) {
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

/// 一段一段去掉漏进来的工具调用，直到没有：每次去最早开头的一段，从开头到它自己的收尾，找不到收尾的去到末尾。去掉过
/// 东西的是 `true`。
fn strip_leaks(text: &mut String) -> bool {
    let mut stripped = false;
    while let Some((start, close)) = LEAKS
        .iter()
        .filter_map(|(open, close)| text.find(open).map(|at| (at, *close)))
        .min_by_key(|(at, _)| *at)
    {
        let end = text
            .get(start..)
            .and_then(|rest| rest.find(close))
            .map_or(text.len(), |at| start + at + close.len());
        text.replace_range(start..end, "");
        stripped = true;
    }
    stripped
}

/// 看起来是空的：只有空白和不可见字符。模型「什么都不想说」时常吐一个零宽空格，`trim` 不认它，发出去是一个空气泡。
/// 只拿来判空，不拿来改正文：零宽连接符夹在表情里是有意义的。
fn blank(text: &str) -> bool {
    text.chars().all(|c| {
        c.is_whitespace()
            || matches!(
                c,
                '\u{200B}'..='\u{200F}'
                    | '\u{2060}'..='\u{2064}'
                    | '\u{FEFF}'
                    | '\u{00AD}'
                    | '\u{180E}'
                    | '\u{2028}'
                    | '\u{2029}'
            )
    })
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

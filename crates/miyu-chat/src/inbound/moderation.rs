//! 违规关键词（`docs/blueprint/chat.md` 第二条「怎么走」第 5 条，`18-通讯平台.md` 第六节）：正文里出现关键词，或者正文里
//! 一段 base64 解出来出现关键词，插一面违规旗，交给线路规程让判官认真查一眼。只插旗，不拦。主人发的不查（施工时定的
//! 第 1 条）。
//!
//! 关键词按子串比，短的 ASCII 词是灾难（旧版 `OD` 一个词 7 天误报 447 次）：词表是数据，改了拿真实聊天记录审一遍。

use super::base64::{decode, segments};
use super::{Clock, Ctx, Flag, Inbound, InboundRule, Standing, Step};

/// 违规关键词的参数。出厂的词表和三个数随桥那一步放进出厂的数据，代码里不写死。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moderation {
    /// 关键词：子串；ASCII 的字母不分大小写，别的字照原样比；空的不算。
    pub keywords: Vec<String>,
    /// 正文里的 base64 怎么解、解出来的怎么筛。
    pub base64: Base64,
}

/// 正文里的 base64 的三个数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Base64 {
    /// 一段至少多少个字符才去解，连末尾的 `=` 一起数。太短的大多是普通的词。
    pub min_chars: usize,
    /// 解出来最多看前多少个字符：可打印的比例、关键词都只看这些。
    pub max_chars: usize,
    /// 可打印的字符（不是控制字符的）至少占几成，千分比：低于它的当乱码，不查。
    pub printable: u16,
}

impl Moderation {
    /// `text` 里出现任何一个关键词没有。
    fn hits(&self, text: &str) -> bool {
        // 按字节比：关键词是完整的 UTF-8，非 ASCII 的字节都不小于 0x80、不受 ASCII 大小写影响，只能从字的开头对上。
        let text = text.as_bytes();
        self.keywords
            .iter()
            .filter(|keyword| !keyword.is_empty())
            .any(|keyword| {
                text.windows(keyword.len())
                    .any(|window| window.eq_ignore_ascii_case(keyword.as_bytes()))
            })
    }
}

impl Base64 {
    /// 一段 base64 解出来要查的字：照 UTF-8 读（读不了的字节换成替换字符），只留前 `max_chars` 个字符；可打印的不够
    /// `printable` 的、空的是 `None`。
    fn reveal(self, segment: &str) -> Option<String> {
        let bytes = decode(segment);
        let shown: String = String::from_utf8_lossy(&bytes)
            .chars()
            .take(self.max_chars)
            .collect();
        let total = shown.chars().count();
        let printable = shown.chars().filter(|c| !c.is_control()).count();
        let enough =
            printable.saturating_mul(1000) >= total.saturating_mul(usize::from(self.printable));
        (total > 0 && enough).then_some(shown)
    }
}

/// 违规关键词这条规则。
pub(super) struct Rule;

impl InboundRule for Rule {
    fn name(&self) -> &str {
        "moderation"
    }

    fn judge(&self, msg: &Inbound, ctx: &Ctx, _clock: Clock) -> Step {
        let moderation = &ctx.moderation;
        let base64 = moderation.base64;
        let hit = msg.said.standing != Standing::Owner
            && (moderation.hits(&msg.text)
                || segments(&msg.text)
                    .into_iter()
                    .filter(|segment| segment.len() >= base64.min_chars)
                    .filter_map(|segment| base64.reveal(segment))
                    .any(|shown| moderation.hits(&shown)));
        match hit {
            true => Step::Flag(Flag::Moderation),
            false => Step::Continue,
        }
    }
}

#[cfg(test)]
mod tests;

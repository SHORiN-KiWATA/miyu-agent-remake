//! 违规关键词（`docs/blueprint/chat.md` 第二条「怎么走」第 5 条，`18-通讯平台.md` 第六节）：正文里出现关键词，或者正文里
//! 的 base64 解出来的字（[`Base64::reveal`]）出现关键词，插一面违规旗，交给线路规程让判官认真查一眼。只插旗，不拦。主人发的
//! 不查（施工时定的第 1 条）。
//!
//! 关键词按子串比，短的 ASCII 词是灾难（旧版 `OD` 一个词 7 天误报 447 次）：词表是数据，改了拿真实聊天记录审一遍。

use super::{Base64, Clock, Ctx, Flag, Inbound, InboundRule, Standing, Step};

/// 违规关键词的参数。出厂的词表和三个数随桥那一步放进出厂的数据，代码里不写死。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moderation {
    /// 关键词：子串；ASCII 的字母不分大小写，别的字照原样比；空的不算。
    pub keywords: Vec<String>,
    /// 正文里的 base64 怎么解、解出来的怎么筛。
    pub base64: Base64,
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

/// 违规关键词这条规则。
pub(super) struct Rule;

impl InboundRule for Rule {
    fn judge(&self, msg: &Inbound, ctx: &Ctx, _clock: Clock) -> Step {
        let moderation = &ctx.moderation;
        // 解出来的几段用换行接着：关键词一行一个、里面没有换行，不会跨两段对上。
        let hit = msg.said.standing != Standing::Owner
            && (moderation.hits(&msg.text)
                || moderation
                    .base64
                    .reveal(&msg.text)
                    .is_some_and(|shown| moderation.hits(&shown)));
        match hit {
            true => Step::Flag(Flag::Moderation),
            false => Step::Continue,
        }
    }
}

#[cfg(test)]
mod tests;

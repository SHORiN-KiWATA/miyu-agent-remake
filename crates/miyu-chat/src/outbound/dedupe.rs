//! 去重（`docs/blueprint/chat.md` 第五条「怎么走」第 3 条，`18-通讯平台.md` Q8）：这一回合已经发过的正文、图，不再发一遍。
//! 只看这一回合（[`super::Sent`] 由外面交进来，换回合清空）；旧版另有一道两分钟内跨回合的，不搬（施工时定的第 1 条）。
//!
//! 正文先归一化（只留字母和数字、转小写）比一字不差，短句也算；不一样的再比两字组的 Jaccard 相似度：同一句话换个说法，
//! 用词高度重叠，两字组便宜又够用。两个数（至少几个两字组、相似度的百分比）由外面交进来（[`super::Outbound`]），出厂的 16、66 照旧版实测（施工时定的第 2 条），在出厂文件里（第八条），测试钉在正好压线的例子上。

use std::collections::BTreeSet;

use miyu_kernel::id::ContentHash;

use super::{OutCtx, OutStep, OutWhy, OutboundRule, Outgoing, Target};

/// 去重这条规则。
pub(super) struct Rule;

impl OutboundRule for Rule {
    fn judge(&self, mut outgoing: Outgoing, target: Target, ctx: &OutCtx) -> OutStep {
        if repeats(&outgoing.text, ctx) {
            if outgoing.images.is_empty() {
                return OutStep::Drop(OutWhy::Repeated);
            }
            outgoing.text.clear();
        }
        // 发过的去掉，同一条里重复的只留第一张；先后照原样。
        let mut seen: BTreeSet<ContentHash> = ctx.sent.images.iter().cloned().collect();
        let pictured = !outgoing.images.is_empty();
        outgoing.images.retain(|image| seen.insert(image.clone()));
        match pictured && outgoing.images.is_empty() && outgoing.text.is_empty() {
            true => OutStep::Drop(OutWhy::Repeated),
            false => OutStep::Continue { outgoing, target },
        }
    }
}

/// `text` 这一回合发过没有：归一化以后不是空的，并且和发过的哪一条一字不差，或者自己的两字组够多、相似度够高。
fn repeats(text: &str, ctx: &OutCtx) -> bool {
    let normalized = normalize(text);
    if normalized.is_empty() {
        return false;
    }
    let grams = bigrams(&normalized);
    let rules = &ctx.outbound;
    ctx.sent.texts.iter().any(|before| {
        let before = normalize(before);
        before == normalized
            || (grams.len() >= rules.min_bigrams
                && similar(&grams, &bigrams(&before), rules.similar))
    })
}

/// 归一化：只留字母和数字（`is_alphanumeric`，汉字也算），转成小写。标点、空白、表情都去掉。
fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// 归一化以后相邻两个字的组合，成集合：重复的只算一个。
fn bigrams(normalized: &str) -> BTreeSet<(char, char)> {
    normalized.chars().zip(normalized.chars().skip(1)).collect()
}

/// Jaccard 相似度（交集 ÷ 并集）不低于 `percent` 个百分点。只在 `mine` 够 `min_bigrams` 个时调，`min_bigrams` 是 0 时
/// 两边都可能是空集，并集是 0：这时当不相似。
fn similar(mine: &BTreeSet<(char, char)>, theirs: &BTreeSet<(char, char)>, percent: u8) -> bool {
    let shared = mine.intersection(theirs).count();
    let union = mine.len() + theirs.len() - shared;
    union > 0 && shared * 100 >= union * usize::from(percent)
}

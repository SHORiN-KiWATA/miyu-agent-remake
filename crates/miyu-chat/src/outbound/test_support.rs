//! 测试共用的几样：造要发的一条、造情形、过自带的链。参数照出厂的数（4 条、15 秒），只在测试里写。

use miyu_kernel::id::ContentHash;

use super::{Out, OutChain, OutCtx, OutWhy, Outbound, Outgoing, Sent, Since, Target};

/// 出厂的「隔几条别人的消息才引用」。
pub(crate) const QUOTE_AFTER: u64 = 4;

/// 出厂的「隔多少毫秒才 @」：15 秒。
pub(crate) const MENTION_AFTER: i64 = 15_000;

/// 一条只有正文、没有图的。
pub(crate) fn text(body: &str) -> Outgoing {
    Outgoing {
        text: body.to_string(),
        images: Vec::new(),
    }
}

/// 一张图的哈希：拿名字当内容算，同一个名字是同一张图。
pub(crate) fn image(name: &str) -> ContentHash {
    ContentHash::of(name.as_bytes())
}

/// 一条有正文、有图的：图照名字算哈希（[`image`]）。
pub(crate) fn with_images(body: &str, images: &[&str]) -> Outgoing {
    Outgoing {
        text: body.to_string(),
        images: images.iter().map(|name| image(name)).collect(),
    }
}

/// 这一回合什么都没发过、不想引用也不想 @、刚进来没人说话的情形。
pub(crate) fn ctx() -> OutCtx {
    OutCtx {
        sent: Sent::default(),
        target: Target::default(),
        since: Since {
            others: 0,
            elapsed: 0,
            last_is_own: false,
        },
        outbound: Outbound {
            quote_after: QUOTE_AFTER,
            mention_after: MENTION_AFTER,
            min_bigrams: 16,
            similar: 66,
        },
    }
}

/// 这一回合发过这几条正文的情形。
pub(crate) fn sent(texts: &[&str]) -> OutCtx {
    let mut ctx = ctx();
    ctx.sent.texts = texts.iter().map(|text| text.to_string()).collect();
    ctx
}

/// 自带的链判一条。
pub(crate) fn judge(outgoing: Outgoing, ctx: &OutCtx) -> Out {
    OutChain::builtin().judge(outgoing, ctx)
}

/// 发这一条，不带引用和 @。
pub(crate) fn send(outgoing: Outgoing) -> Out {
    Out::Send {
        outgoing,
        target: Target::default(),
    }
}

/// 丢掉。
pub(crate) fn drop(why: OutWhy) -> Out {
    Out::Drop(why)
}

/// `n` 个各不相同的汉字，从第 `from` 个起：它们相邻两个字的组合各不相同，好凑出想要的两字组个数和相似度。
pub(crate) fn distinct(from: u32, n: u32) -> String {
    (from..from + n)
        .filter_map(|i| char::from_u32(0x4E00 + i))
        .collect()
}

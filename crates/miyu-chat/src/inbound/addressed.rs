//! 冲她来（`docs/blueprint/chat.md` 第二条「怎么走」第 11 条，`18-通讯平台.md` 第七节，施工 O-12 下）：一条消息是不是对她
//! 说的，填 [`Said::addressed`](crate::Said::addressed)。限流满了只给冲她来的回一句，主动回复判断给冲她来的加 `direct` 分。
//!
//! 照旧版（`group_trigger_text`）：触发词只认开头，不认中间。中间出现就算的话，「为什么」这类词设成触发词会误叫；旧版
//! 08-29 实测过（施工时定的第 16 条）。私聊不看触发词，一律算（施工时定的第 17 条）。

use crate::VenueKind;

/// 一条消息是不是冲她来的（「怎么走」第 11 条）：
///
/// - 私聊（`kind` 是 [`VenueKind::Private`]）：一律是。
/// - 群：@ 了她（`mentions_me`）是；引用的是她的消息（`quotes_me`）是；正文 `text` 去掉开头的空白以后，以 `keywords` 里
///   某个触发词开头是。
///
/// `keywords` 是场所规则的触发词：区分大小写，空的不算，原样比（不去空白）。名字也算触发词，写在里面，出厂是空的；这里
/// 不自动加上人格的名字。
pub fn addressed(
    kind: VenueKind,
    mentions_me: bool,
    quotes_me: bool,
    text: &str,
    keywords: &[String],
) -> bool {
    match kind {
        VenueKind::Private => true,
        VenueKind::Group => {
            let text = text.trim_start();
            mentions_me
                || quotes_me
                || keywords
                    .iter()
                    .any(|keyword| !keyword.is_empty() && text.starts_with(keyword.as_str()))
        }
    }
}

#[cfg(test)]
mod tests;

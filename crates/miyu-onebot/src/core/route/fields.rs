//! `session.send` 的场所的格（`onebot.md` 第一条「群消息」第 6 条、第 8 条，`venues.md`「场所的格」，施工 O-22）：一条
//! 消息的平台编号、发的人的名字、引用、@、带的东西，加上旁听、睡着、看不看得到号。
//!
//! 照核心的写法洗：核心收到写错的格整条 `bad_params`、什么都不记，一个怪名字、一个长编号不能让整条消息丢了（「施工时定的」
//! 第 63 条）。名字去掉控制字符、截到上限，空白的不写；引用、带的东西的编号不合的那一格、那一样不记。假的、空的格不写。

use miyu_kernel::id::ExternalId;
use serde_json::{Map, Value, json};

use crate::onebot::Posted;

/// 平台的编号最长几个字符（`venues.md`「场所的格」的 `msg`、`reply_to`，带的东西的 `id`）。
const ID: usize = 128;

/// 发的人的名字最长几个字符（`name`）。
pub(super) const NAME: usize = 64;

/// 带的东西的名字最长几个字符（文件名、表情的字）。
const MEDIA_NAME: usize = 200;

/// 几个开关：不写的是假。
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Flags {
    /// @ 了她。
    pub(super) mentions_me: bool,
    /// 旁听：群消息一律是（「群消息」第 6 条）。
    pub(super) ambient: bool,
    /// 睡着时收到的。
    pub(super) asleep: bool,
    /// 渲染时写发的人的平台身份。
    pub(super) show_ids: bool,
}

/// 消息 `posted` 的场所的格：@ 了的别人是 `mentions`（她自己的在 `flags.mentions_me`），开关照 `flags`。
pub(super) fn fields(posted: &Posted, mentions: &[ExternalId], flags: Flags) -> Value {
    let segments = &posted.segments;
    let mut venue = Map::new();
    venue.insert("msg".into(), json!(posted.message_id.to_string()));
    if let Some(name) = posted.name.as_deref().and_then(|name| clean(name, NAME)) {
        venue.insert("name".into(), json!(name));
    }
    if let Some(reply_to) = segments.reply_to.as_deref().filter(|id| fits(id)) {
        venue.insert("reply_to".into(), json!(reply_to));
    }
    if !mentions.is_empty() {
        venue.insert("mentions".into(), json!(mentions));
    }
    let media: Vec<Value> = segments
        .media
        .iter()
        .filter(|media| fits(&media.id))
        .map(|media| {
            let mut one = json!({"kind": media.kind.as_str(), "id": media.id});
            if let Some(name) = media
                .name
                .as_deref()
                .and_then(|name| clean(name, MEDIA_NAME))
            {
                one["name"] = json!(name);
            }
            one
        })
        .collect();
    if !media.is_empty() {
        venue.insert("media".into(), Value::Array(media));
    }
    for (key, on) in [
        ("mentions_me", flags.mentions_me),
        ("mentions_all", segments.mentions_all),
        ("ambient", flags.ambient),
        ("asleep", flags.asleep),
        ("show_ids", flags.show_ids),
    ] {
        if on {
            venue.insert(key.into(), json!(true));
        }
    }
    Value::Object(venue)
}

/// 洗一个名字：去掉控制字符，截到 `most` 个字符（不截断一个字）；去掉首尾空白是空的交回空的。
pub(super) fn clean(name: &str, most: usize) -> Option<String> {
    let cleaned: String = name
        .chars()
        .filter(|char| !char.is_control())
        .take(most)
        .collect();
    (!cleaned.trim().is_empty()).then_some(cleaned)
}

/// 一个平台的编号合不合核心的写法：不空，最多 [`ID`] 个字符，没有控制字符。
fn fits(id: &str) -> bool {
    !id.is_empty() && id.chars().count() <= ID && !id.chars().any(char::is_control)
}

#[cfg(test)]
mod tests;

//! 群里的一行（施工 O-13 中，`docs/designs/18-通讯平台.md` 第九节，格式照旧版的群聊记录）：群会话里带 `venue`、不旁听的
//! `message.user` 渲染成一行一条，替掉原来的正文。
//!
//! `[14:02] 小林 (id=qq:10086, manager) [msg=8810]: 内容`，下面可选缩进的 `reply-to:`、`@mentions:` 两行。钟点照会话钉下的
//! 时区；`id=` 只在这一条的 `show_ids` 是真时写；身份只写 `owner`（主人对应表里有的）、`manager`（桥报的）。名字、正文、
//! 编号、带的东西的名字是不可信的，照模板的规矩转义成一行（`template::escape`）：伪造不出另一条记录；钟点、身份、`@all`、
//! `[you]` 是可信的，原样。图片、文件块照旧接在这一行后面交给驱动。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Media, VenueMessage};
use miyu_kernel::id::ExternalId;
use miyu_kernel::origin::{By, Role};
use miyu_kernel::template::escape;
use miyu_kernel::time::Timestamp;

use crate::texts::GroupChat;

/// 正文最多留多少字节（照旧版）：超了的在字的边界上截掉后面的。
const TEXT_LIMIT: usize = 4096;

/// 群里的人在 `at` 说的一条：`by` 是谁，`venue` 是平台的格，`blocks` 是内容块。交回这一行，后面接着字以外的块。
pub(crate) fn line(
    at: Timestamp,
    by: &By,
    venue: &VenueMessage,
    blocks: Vec<Block>,
    texts: &GroupChat,
) -> Vec<Block> {
    let (words, rest): (Vec<Block>, Vec<Block>) = blocks
        .into_iter()
        .partition(|block| matches!(block, Block::Text(_)));
    let mut line = format!(
        "[{}] {} [msg={}]: {}",
        at.local_clock(texts.offset),
        sender(by, venue),
        escape(&venue.msg),
        content(&words, &venue.media, &texts.no_text),
    );
    if let Some(reply_to) = &venue.reply_to {
        line.push_str("\n  reply-to: msg=");
        line.push_str(&escape(reply_to));
    }
    if let Some(mentions) = mentions(venue) {
        line.push_str("\n  @mentions: ");
        line.push_str(&mentions);
    }
    let mut out = vec![Block::Text(Text { text: line })];
    out.extend(rest);
    out
}

/// 发的人：名字（没有的写平台身份），后面括号里是 `id=`（这一条 `show_ids`、又有名字的才写）和身份（`owner`、`manager`）。
fn sender(by: &By, venue: &VenueMessage) -> String {
    let (id, role) = match by {
        By::External(external) => {
            let role = match (&external.account, external.role) {
                (Some(_), _) => Some("owner"),
                (None, Some(Role::Manager)) => Some("manager"),
                (None, _) => None,
            };
            (Some(&external.id), role)
        }
        // 群里的主人照理是外部身份；经对应表认出的本人也写 `owner`。
        By::Person(person) => (person.via.as_ref(), Some("owner")),
        _ => (None, None),
    };
    let name = venue
        .name
        .as_deref()
        .or(id.map(ExternalId::as_str))
        .unwrap_or_default();
    let mut notes = Vec::new();
    if let (true, Some(_), Some(id)) = (venue.show_ids, &venue.name, id) {
        notes.push(format!("id={}", escape(id.as_str())));
    }
    notes.extend(role.map(str::to_string));
    if notes.is_empty() {
        escape(name)
    } else {
        format!("{} ({})", escape(name), notes.join(", "))
    }
}

/// 内容：正文去掉前后空白、截到 [`TEXT_LIMIT`] 字节，带的东西空一格接在后面；都没有的写 `no_text`。
fn content(words: &[Block], media: &[Media], no_text: &str) -> String {
    let joined = words
        .iter()
        .filter_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut parts = Vec::new();
    let text = cut(joined.trim(), TEXT_LIMIT);
    if !text.is_empty() {
        parts.push(escape(text));
    }
    parts.extend(media.iter().map(|item| match &item.name {
        Some(name) => format!("[{}: {}]", item.kind.as_str(), escape(name)),
        None => format!("[{}]", item.kind.as_str()),
    }));
    if parts.is_empty() {
        no_text.to_string()
    } else {
        parts.join(" ")
    }
}

/// 缩进的 @ 那一行的内容：`@all`（@ 了全体成员）、`[you]`（@ 了她），看得到身份的再列 @ 了的人的平台身份；都没有的没有。
/// 看不到身份的，@ 了的人的名字在正文里（桥写成 `@名字`），不另写。
fn mentions(venue: &VenueMessage) -> Option<String> {
    let mut items = Vec::new();
    if venue.mentions_all {
        items.push("@all".to_string());
    }
    if venue.mentions_me {
        items.push("[you]".to_string());
    }
    if venue.show_ids {
        items.extend(venue.mentions.iter().map(|id| escape(id.as_str())));
    }
    (!items.is_empty()).then(|| items.join(", "))
}

/// `text` 最多留 `limit` 字节，截在字的边界上。
fn cut(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests;

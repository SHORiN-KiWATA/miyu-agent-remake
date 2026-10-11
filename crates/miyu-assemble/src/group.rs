//! 群里的一行（施工 O-13 中，`docs/designs/18-通讯平台.md` 第九节，格式照旧版的群聊记录）：群会话里带 `venue`、不旁听的
//! `message.user` 渲染成一行一条，替掉原来的正文。
//!
//! `[14:02] 小林 (id=qq:10086, manager) [msg=8810]: 内容`，下面可选缩进的 `reply-to:`、`@mentions:` 两行。钟点照会话钉下的
//! 时区；`id=` 只在这一条的 `show_ids` 是真时写；身份只写 `owner`（主人对应表里有的）、`manager`（桥报的）。名字、正文、
//! 编号、带的东西的名字是不可信的，照模板的规矩转义成一行（`template::escape`）：伪造不出另一条记录；钟点、身份、`@all`、
//! `[you]` 是可信的，原样。图片、文件块照旧接在这一行后面交给驱动。
//!
//! 带的东西的记号（施工 O-33）：`[种类 #第几个: 名字, 大小]`，大小照 `size.rs` 写；O-33 起造的群会话（快照里有语音那一句）
//! 一条里不止一样的照先后从 1 数、每样标 `#n`（和 `fetch_media` 的 `index` 一个数法），语音后面接那一句；以前造的不标、
//! 不接，前缀一个字节不变。
//!
//! 开一轮的那条前面的群聊近况在 `recent.rs`（施工 O-13 下）：两次触发之间的旁听、别的线替她发的话，一行一条。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Event, Media, MediaKind, VenueDelivered, VenueMessage};
use miyu_kernel::id::ExternalId;
use miyu_kernel::origin::{By, Role};
use miyu_kernel::template::escape;
use miyu_kernel::time::Timestamp;

use crate::size::readable;
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
    let line = record(at, by, venue, &words, texts, "");
    let mut out = vec![Block::Text(Text { text: line })];
    out.extend(rest);
    out
}

/// 一条的字：`[时刻] 发的人 [msg=编号]<mark>: 内容`，下面可选缩进的两行。`mark` 是编号后面的可信记号（撤回的，`recent.rs`），
/// 没有的是空的。`words` 里只看文本块。
fn record(
    at: Timestamp,
    by: &By,
    venue: &VenueMessage,
    words: &[Block],
    texts: &GroupChat,
    mark: &str,
) -> String {
    let mut line = format!(
        "[{}] {} [msg={}]{mark}: {}",
        at.local_clock(texts.offset),
        sender(by, venue),
        escape(&venue.msg),
        content(words, &venue.media, texts),
    );
    if let Some(reply_to) = &venue.reply_to {
        line.push_str("\n  reply-to: msg=");
        line.push_str(&escape(reply_to));
    }
    if let Some(mentions) = mentions(venue) {
        line.push_str("\n  @mentions: ");
        line.push_str(&mentions);
    }
    line
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

/// 内容：正文去掉前后空白、截到 [`TEXT_LIMIT`] 字节，带的东西空一格接在后面（[`markers`]）；都没有的写 `no_text`。
fn content(words: &[Block], media: &[Media], texts: &GroupChat) -> String {
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
    parts.extend(markers(media, texts.voice.as_deref()));
    if parts.is_empty() {
        texts.no_text.clone()
    } else {
        parts.join(" ")
    }
}

/// 一条带的东西的记号，照先后。`voice` 是语音那一句：有它的（O-33 起造的群会话）不止一样的标第几个、语音接这一句；
/// 没有的照以前写。
fn markers(media: &[Media], voice: Option<&str>) -> Vec<String> {
    let numbered = voice.is_some() && media.len() > 1;
    media
        .iter()
        .zip(1..)
        .map(|(item, number)| {
            let mut marker = marker(item, numbered.then_some(number));
            if let (MediaKind::Voice, Some(voice)) = (&item.kind, voice) {
                marker.push(' ');
                marker.push_str(voice);
            }
            marker
        })
        .collect()
}

/// 带的一样东西的记号：`[image]`；标第几个的 `[image #2]`；有名字、大小的冒号后面逗号隔开，`[file: 名字, 1.2 MB]`。
/// 名字照模板的规矩转义。
fn marker(item: &Media, number: Option<usize>) -> String {
    let mut head = item.kind.as_str().to_string();
    if let Some(number) = number {
        head.push_str(&format!(" #{number}"));
    }
    let details: Vec<String> = item
        .name
        .as_deref()
        .map(escape)
        .into_iter()
        .chain(item.size.map(readable))
        .collect();
    if details.is_empty() {
        format!("[{head}]")
    } else {
        format!("[{head}: {}]", details.join(", "))
    }
}

/// 不在群里的一条场所消息、没有内容块的（施工 O-13 补）：带的东西的记号一个文本块，空一格隔开；带的东西也没有的，没有。
/// 不标第几个、语音不接那一句（施工 O-33：`fetch_media` 只给群，私聊没有群会话的字），只多大小。
pub(crate) fn bare(venue: &VenueMessage) -> Option<Block> {
    let markers = markers(&venue.media, None);
    (!markers.is_empty()).then(|| {
        Block::Text(Text {
            text: markers.join(" "),
        })
    })
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

/// 撤回的记号：发的人自己撤的、看不到身份的写 ` (recalled)`；别人撤的、看得到身份的写是谁撤的。
fn recall_mark(sender: &By, by: &ExternalId, show_ids: bool) -> String {
    let own = matches!(sender, By::External(external) if external.id == *by);
    if own || !show_ids {
        " (recalled)".to_string()
    } else {
        format!(" (recalled by {})", escape(by.as_str()))
    }
}

/// 别的线替她发进群里的一条：`[时刻] [you] [msg=编号]: 正文`，带的图每张一个 `[image]`。
fn you(event: &Event, delivered: &VenueDelivered, texts: &GroupChat) -> String {
    let mut parts = Vec::new();
    let text = cut(delivered.text.trim(), TEXT_LIMIT);
    if !text.is_empty() {
        parts.push(escape(text));
    }
    parts.extend(delivered.images.iter().map(|_| "[image]".to_string()));
    let content = if parts.is_empty() {
        texts.no_text.clone()
    } else {
        parts.join(" ")
    };
    format!(
        "[{}] [you] [msg={}]: {content}",
        event.at.local_clock(texts.offset),
        escape(&delivered.msg),
    )
}

mod recent;
mod records;

pub(crate) use recent::recent;
pub use records::{Records, records};

#[cfg(test)]
mod tests;

//! 认消息段（`onebot.md` 第一条「群消息」第 2 条，施工 O-22）：消息是段的数组时一段一段认成正文（字、@、占位）、引用、
//! @全体、带的东西；是字符串（CQ 码）时照第 6 条只读字（「施工时定的」第 62 条）。
//!
//! 只认、不洗：名字、编号合不合核心的写法（长短、控制字符）由交给核心的一方照 `venues.md`「场所的格」洗
//! （`core/route/fields.rs`）。不下载，带的东西只记平台的编号（懒下载，18 第五节）。

use serde_json::Value;

use super::{number, text_of};

/// 合并转发的占位：照核心渲染带的东西的写法 `[种类]`（「施工时定的」第 61 条），不展开。
const FORWARD: &str = "[forward]";

/// 卡片（`json`、`xml` 段）的占位，同 [`FORWARD`]。
const CARD: &str = "[card]";

/// 带的东西的平台编号从这几格取，照先后，头一个有的：NapCat 的图、语音、视频、文件是 `file_id` 或 `file`，小黄脸是 `id`，
/// 商城表情是 `emoji_id`。
const IDS: [&str; 4] = ["file_id", "file", "id", "emoji_id"];

/// 文件的名字从这几格取，照先后，头一个有的。
const FILE_NAMES: [&str; 3] = ["name", "file_name", "file"];

/// 一条消息认出来的段。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Segments {
    /// 正文，照段的先后：字（占位也是字）和 @ 了谁。
    pub pieces: Vec<Piece>,
    /// 引用的那一条的平台编号：几段引用的只认第一段。
    pub reply_to: Option<String>,
    /// @ 了全体成员（`at` 段的 `qq` 是 `all`）：正文里不写（「施工时定的」第 60 条）。
    pub mentions_all: bool,
    /// 带的东西，照段的先后。
    pub media: Vec<Media>,
}

/// 正文的一块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// 字，照原样；合并转发、卡片的占位也是这一种。
    Text(String),
    /// @ 了号是它的人：交给核心以前写成 `@名字`（「群消息」第 5 条）。
    At(i64),
}

/// 带的一样东西。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    /// 什么种类。
    pub kind: MediaKind,
    /// 平台的编号：不空。
    pub id: String,
    /// 文件名、表情的字：有的话，去掉首尾空白不空的才有。
    pub name: Option<String>,
}

/// 带的东西的种类，和核心认的五种一样（`venues.md`「场所的格」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    /// 图片。
    Image,
    /// 文件。
    File,
    /// 语音（`record` 段）。
    Voice,
    /// 视频。
    Video,
    /// 表情：表情包、QQ 的小黄脸、商城表情。
    Sticker,
}

impl MediaKind {
    /// 交给核心的写法。
    pub fn as_str(self) -> &'static str {
        match self {
            MediaKind::Image => "image",
            MediaKind::File => "file",
            MediaKind::Voice => "voice",
            MediaKind::Video => "video",
            MediaKind::Sticker => "sticker",
        }
    }
}

impl Segments {
    /// @ 了的号，照先后、不重。
    pub fn at(&self) -> Vec<i64> {
        let mut all = Vec::new();
        for piece in &self.pieces {
            if let Piece::At(user) = piece
                && !all.contains(user)
            {
                all.push(*user);
            }
        }
        all
    }

    /// 认一段：`kind` 是段的 `type`，`data` 是它的 `data`。不认识的不记。
    fn take(&mut self, kind: &str, data: &Value) {
        match kind {
            "text" => {
                if let Some(text) = data["text"].as_str() {
                    self.pieces.push(Piece::Text(text.to_string()));
                }
            }
            "at" if data["qq"] == "all" => self.mentions_all = true,
            "at" => {
                if let Some(user) = number(&data["qq"]) {
                    self.pieces.push(Piece::At(user));
                }
            }
            "reply" => {
                if self.reply_to.is_none() {
                    self.reply_to = id(&data["id"]);
                }
            }
            "forward" => self.pieces.push(Piece::Text(FORWARD.to_string())),
            "json" | "xml" => self.pieces.push(Piece::Text(CARD.to_string())),
            _ => self.media.extend(media(kind, data)),
        }
    }
}

/// 认一条消息的 `message`：段的数组一段一段认；字符串（CQ 码）照第 6 条只读字（[`text_of`]）；别的什么都没有。
pub fn segments(message: &Value) -> Segments {
    let Value::Array(all) = message else {
        let text = text_of(message);
        let pieces = match text.is_empty() {
            true => Vec::new(),
            false => vec![Piece::Text(text)],
        };
        return Segments {
            pieces,
            ..Segments::default()
        };
    };
    let mut segments = Segments::default();
    for segment in all {
        segments.take(
            segment["type"].as_str().unwrap_or_default(),
            &segment["data"],
        );
    }
    segments
}

/// 带的东西的一段：种类照段（图片里的表情包、商城表情记成表情），编号照 [`IDS`]；不是带东西的段、一个编号都没有的不记。
fn media(kind: &str, data: &Value) -> Option<Media> {
    let (kind, name) = match kind {
        // NapCat 把商城表情发成 `image`，带 `emoji_id`，`summary` 是表情的字。
        "image" if !data["emoji_id"].is_null() => (MediaKind::Sticker, words(&data["summary"])),
        // `sub_type` 是 1 的是表情包；它的 `summary` 是「[动画表情]」这类，不是表情的字。
        "image" if number(&data["sub_type"]) == Some(1) => (MediaKind::Sticker, None),
        "image" => (MediaKind::Image, None),
        "mface" => (MediaKind::Sticker, words(&data["summary"])),
        "face" => (MediaKind::Sticker, words(&data["raw"]["faceText"])),
        "record" => (MediaKind::Voice, None),
        "video" => (MediaKind::Video, None),
        "file" => (
            MediaKind::File,
            FILE_NAMES.iter().find_map(|key| words(&data[*key])),
        ),
        _ => return None,
    };
    let id = IDS.iter().find_map(|key| id(&data[*key]))?;
    Some(Media { kind, id, name })
}

/// 一格编号：不空的字，或者整数写成十进制的字。
fn id(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// 一格名字：去掉首尾空白不空的字，照原样。
fn words(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string)
}

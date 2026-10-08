//! `session.send` 的 `venue`（施工 O-13 上，`docs/blueprint/chat.md` 第七条第 2 条）：通讯平台上的一条消息的那几格，查过写法
//! 换成内核的 [`VenueMessage`]，原样记进 `message.user`。写错的整条 `bad_params`。

use serde::Deserialize;

use miyu_kernel::event::{Media, MediaKind, VenueMessage};
use miyu_kernel::id::ExternalId;

use crate::refusal::Refusal;

/// 平台给的编号最长几个字符。
const ID: usize = 128;
/// 发的人的名字最长几个字符。
const NAME: usize = 64;
/// 文件名、表情的字最长几个字符。
const MEDIA_NAME: usize = 200;

/// `venue` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VenueMessageParams {
    msg: String,
    #[serde(default)]
    reply_to: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    mentions: Vec<String>,
    #[serde(default)]
    mentions_me: bool,
    #[serde(default)]
    mentions_all: bool,
    #[serde(default)]
    media: Vec<MediaParams>,
    #[serde(default)]
    ambient: bool,
    #[serde(default)]
    asleep: bool,
    #[serde(default)]
    show_ids: bool,
}

/// 一样带的东西。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MediaParams {
    kind: MediaKindParam,
    id: String,
    #[serde(default)]
    name: Option<String>,
}

/// 带的东西是哪一种：只认这五种。
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MediaKindParam {
    Image,
    File,
    Voice,
    Video,
    Sticker,
}

impl VenueMessageParams {
    /// 查过写法，换成内核的那一份。
    pub(crate) fn checked(self) -> Result<VenueMessage, Refusal> {
        let media = self
            .media
            .into_iter()
            .map(|media| {
                Ok(Media {
                    kind: match media.kind {
                        MediaKindParam::Image => MediaKind::Image,
                        MediaKindParam::File => MediaKind::File,
                        MediaKindParam::Voice => MediaKind::Voice,
                        MediaKindParam::Video => MediaKind::Video,
                        MediaKindParam::Sticker => MediaKind::Sticker,
                    },
                    id: platform_id(media.id)?,
                    name: media.name.map(|name| words(name, MEDIA_NAME)).transpose()?,
                })
            })
            .collect::<Result<Vec<_>, Refusal>>()?;
        Ok(VenueMessage {
            msg: platform_id(self.msg)?,
            reply_to: self.reply_to.map(platform_id).transpose()?,
            name: self.name.map(|name| words(name, NAME)).transpose()?,
            mentions: self
                .mentions
                .iter()
                .map(|who| ExternalId::parse(who).map_err(|_| Refusal::BAD_PARAMS))
                .collect::<Result<_, _>>()?,
            mentions_me: self.mentions_me,
            mentions_all: self.mentions_all,
            media,
            ambient: self.ambient,
            asleep: self.asleep,
            show_ids: self.show_ids,
        })
    }
}

/// 平台给的编号：不是空的，最多 [`ID`] 个字符，没有控制字符（`events.append` 的场所事件也照它查）。
pub(crate) fn platform_id(text: String) -> Result<String, Refusal> {
    let fits =
        !text.is_empty() && text.chars().count() <= ID && !text.chars().any(char::is_control);
    fits.then_some(text).ok_or(Refusal::BAD_PARAMS)
}

/// 给人看的一句：去掉前后空白不是空的，最多 `most` 个字符，没有控制字符。原样记。
fn words(text: String, most: usize) -> Result<String, Refusal> {
    let fits = !text.trim().is_empty()
        && text.chars().count() <= most
        && !text.chars().any(char::is_control);
    fits.then_some(text).ok_or(Refusal::BAD_PARAMS)
}

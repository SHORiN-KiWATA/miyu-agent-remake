//! 场所的事件（施工 O-13 上，`docs/blueprint/chat.md` 第七条第 2 条）：`message.user` 的 `venue` 格，桥记的 `venue.recalled`、
//! `venue.delivered`。内核不解读平台的编号，原样记；群聊近况怎么渲染它们在组装那一层（O-13 下）。

use serde::{Deserialize, Serialize};

use crate::id::{ContentHash, ExternalId, SessionId, TurnId};
use crate::text_enum::text_enum;

/// `message.user` 的 `venue`：通讯平台上的一条消息。旁听的（`ambient`）只记下，不开回合，回合进行中也不排进这一轮。
/// 假的、空的、没写的不写。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VenueMessage {
    /// 平台给的消息编号。
    pub msg: String,
    /// 引用的那一条的平台编号。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// 发的人此刻在这个场所里叫什么（群名片，没有的用昵称）。名字会变，每条消息各记各的。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// @ 了谁：平台身份。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<ExternalId>,
    /// @ 了她。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mentions_me: bool,
    /// @ 了全体成员：不算 @ 了她。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mentions_all: bool,
    /// 带的图、文件、语音、视频、表情：只记编号，要看时再由桥去取，不进内容块。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub media: Vec<Media>,
    /// 旁听：不冲她来的，只记下。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ambient: bool,
    /// 睡着时收到的：不进群聊近况。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub asleep: bool,
}

/// 一条消息带的一样东西。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Media {
    /// 哪一种。
    pub kind: MediaKind,
    /// 平台给的编号。
    pub id: String,
    /// 文件的文件名、表情的字；语音、视频没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

text_enum!(
    /// 带的东西是哪一种。
    MediaKind {
        /// 图。
        Image = "image",
        /// 文件。
        File = "file",
        /// 语音。
        Voice = "voice",
        /// 视频。
        Video = "video",
        /// 表情（平台的小黄脸、商城表情都算）。
        Sticker = "sticker",
    }
);

/// `venue.recalled`：有人撤回了一条消息。不叫 `withdrawn`：内核的 `message.withdrawn` 是排着队的消息退回给头。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VenueRecalled {
    /// 被撤的那一条的平台编号。
    pub msg: String,
    /// 谁撤的。
    pub by: ExternalId,
}

/// `venue.delivered`：她的一句话实际发到了平台上（出站链洗过的）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VenueDelivered {
    /// 哪条线：主线或支线的会话编号。
    pub line: SessionId,
    /// 哪一轮。
    pub turn: TurnId,
    /// 回的是谁。
    pub to: Vec<ExternalId>,
    /// 平台给的编号。
    pub msg: String,
    /// 发出去的正文。
    pub text: String,
    /// 带的图的内容哈希。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ContentHash>,
}

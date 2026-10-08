//! OneBot v11 这一头（`onebot.md` 第一条「怎么走」第 4 到 6 条、第 10 条）：NapCat 发来的一帧认成什么（回应、私聊、别的
//! 事件），私聊里的文字怎么读出来（`text`），发出去的动作和回应怎么照 `echo` 配对（`calls`），`send_private_msg` 写成什么样。
//!
//! 号（机器人的号、对方的号、消息编号）和时刻照 OneBot 是整数；有的实现写成字符串，也认。
//!
//! 平台的名字 [`PLATFORM`] 只写在这里：场所、平台上的人、命令编号都照它拼（第 7、8 条）。场所、平台上的人经群聊内核拼，
//! 桥不手拼（`chat.md` 第七条第 1 条）。

mod calls;
mod text;

use miyu_chat::{Venue, VenueKind};
use miyu_kernel::FormatError;
use miyu_kernel::id::{ExternalId, VenueId};
use serde_json::{Value, json};

pub use calls::{CallError, Calls, Pending};
pub use text::text_of;

/// 平台的名字：场所、平台上的人、命令编号的头一段（第 7、8 条）。
pub const PLATFORM: &str = "qq";

/// 和号 `user` 的私聊这个场所：经群聊内核的 [`Venue::new`] 拼成 `qq:private:<号>`（第 7 条，`chat.md` 第七条第 1 条）。
///
/// # Errors
///
/// 群聊内核拼不出来，报它的 [`FormatError`]。号是整数，照说不会；用的一方照实记下，不 `unwrap`。
pub fn private_venue(user: i64) -> Result<VenueId, FormatError> {
    Ok(Venue::new(PLATFORM, VenueKind::Private, &user.to_string())?
        .id()
        .clone())
}

/// 平台上号是 `user` 的那个人：经群聊内核的 [`miyu_chat::person`] 拼成 `qq:<号>`（第 7、8 条：`venue.session` 的
/// `peer`、`session.send` 的 `as.external`）。
///
/// # Errors
///
/// 同 [`private_venue`]。
pub fn person(user: i64) -> Result<ExternalId, FormatError> {
    miyu_chat::person(PLATFORM, &user.to_string())
}

/// 机器人号 `bot` 收进来的第 `message_id` 条消息、平台给的时刻是 `time`，发进去用的命令编号：
/// `qq:<机器人的号>:<消息编号>:<时刻>`（第 8 条，`chat.md` 第七条第 1 条）。带上时刻：NapCat 重置本地库以后消息编号会重号，
/// 同一个编号、不同的时刻是两条，新消息不会被当成重发吞掉。斜杠命令交 `command.run` 也用它（O-19，「斜杠命令」第 1 条）：
/// 一条消息要么是命令、要么是话，编号只用一次。
pub fn command_id(bot: i64, message_id: i64, time: i64) -> String {
    format!("{PLATFORM}:{bot}:{message_id}:{time}")
}

/// 一条私聊的文字消息：只看 `post_type = message`、`message_type = private` 的（第 5 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Private {
    /// 收到它的机器人的号（事件的 `self_id`）。
    pub bot: i64,
    /// 对方的号（`user_id`）。
    pub user: i64,
    /// 消息编号（`message_id`）：拼进命令编号，断线重发、平台重发都只收一次（第 8 条）。
    pub message_id: i64,
    /// 平台给的时刻（事件的 `time`，整数秒）：也拼进命令编号（第 8 条）。没带、读不出的是 0（「施工时定的」第 20 条）：消息
    /// 照送，去重退回只看消息编号。
    pub time: i64,
    /// 读出来的文字（第 6 条），照原样，没去掉首尾空白。
    pub text: String,
}

/// NapCat 发来的一帧。
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    /// 动作的回应：带 `echo`、不是事件。
    Reply(Value),
    /// 一条私聊。
    Private(Private),
    /// 别的事件（群消息、通知、请求、心跳、生命周期……）或者读不懂的：`post_type`，没有的是空字。
    Other(String),
}

/// 认一帧。
pub fn read(frame: Value) -> Frame {
    let kind = frame["post_type"].as_str().unwrap_or_default().to_string();
    if kind.is_empty() && frame.get("echo").is_some() {
        return Frame::Reply(frame);
    }
    if kind != "message" || frame["message_type"] != "private" {
        return Frame::Other(kind);
    }
    let (Some(bot), Some(user), Some(message_id)) = (
        number(&frame["self_id"]),
        number(&frame["user_id"]),
        number(&frame["message_id"]),
    ) else {
        return Frame::Other(kind);
    };
    Frame::Private(Private {
        bot,
        user,
        message_id,
        time: number(&frame["time"]).unwrap_or(0),
        text: text_of(&frame["message"]),
    })
}

/// 事件里机器人的号（`self_id`）：连进来时没带 `X-Self-ID` 的，照第一条事件的认（第 2 条）。
pub fn self_id(frame: &Value) -> Option<i64> {
    number(&frame["self_id"])
}

/// 一个号或者时刻：整数，或者写成整数的字符串。
pub fn number(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
}

/// `send_private_msg` 的参数：发给 `user`，一个文字段 `text`（第 10 条）。
pub fn private_message(user: i64, text: &str) -> Value {
    json!({"user_id": user, "message": [{"type": "text", "data": {"text": text}}]})
}

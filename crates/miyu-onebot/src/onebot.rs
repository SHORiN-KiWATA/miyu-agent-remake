//! OneBot v11 这一头（`onebot.md` 第一条「怎么走」第 4 到 6 条、第 10 条，「群消息」「撤回」「出站队列」「好友请求」）：NapCat
//! 发来的一帧认成什么（回应、私聊、群消息、撤回、她被禁言和解禁（施工 O-25 中）、好友请求和群邀请（施工 O-27）、别的事件），
//! 私聊里的文字怎么读出来（`text`），
//! 消息段怎么认（`segments`，施工 O-22），群成员叫什么（`members`，施工 O-22），发出去的动作和回应怎么照 `echo` 配对
//! （`calls`），`send_private_msg`、`send_group_msg`（施工 O-25 上：第一段能带引用和 @）、`delete_msg`（施工 O-25 上）、
//! `set_msg_emoji_like`（施工 O-25 下）、`set_friend_add_request`（施工 O-27）、`set_group_ban`、`group_poke`、`friend_poke`
//! （施工 O-31）写成什么样；取带的东西的 `get_msg`、`get_image`、`get_file`（施工 O-33，`media`）。
//!
//! 号（机器人的号、对方的号、消息编号）和时刻照 OneBot 是整数；有的实现写成字符串，也认。
//!
//! 平台的名字 [`PLATFORM`] 只写在这里：场所、平台上的人、命令编号都照它拼（第 7、8 条）。场所、平台上的人经群聊内核拼，
//! 桥不手拼（`chat.md` 第七条第 1 条）。

mod calls;
pub mod media;
mod members;
mod segments;
mod text;

use miyu_chat::{Venue, VenueKind};
use miyu_kernel::FormatError;
use miyu_kernel::id::{ExternalId, VenueId};
use serde_json::{Value, json};

pub use calls::{CallError, Calls, DETAIL, Pending, said};
pub use members::{MEMBER_INFO, Members, Rank, display_name, member_info, rank_of};
pub use segments::{Fetch, Media, MediaKind, Piece, Segments, segments};
pub use text::text_of;

/// 平台的名字：场所、平台上的人、命令编号的头一段（第 7、8 条）。
pub const PLATFORM: &str = "qq";

/// 和号 `user` 的私聊这个场所：经群聊内核的 [`Venue::new`] 拼成 `qq:private:<号>`（第 7 条，`chat.md` 第七条第 1 条）。
///
/// # Errors
///
/// 群聊内核拼不出来，报它的 [`FormatError`]。号是整数，照说不会；用的一方照实记下，不 `unwrap`。
pub fn private_venue(user: i64) -> Result<VenueId, FormatError> {
    Ok(venue(VenueKind::Private, user)?.id().clone())
}

/// 种类是 `kind`、号是 `number`（私聊的对方、群号）的场所：经群聊内核的 [`Venue::new`] 拼（施工 O-22：套场所规则要
/// [`Venue`]，编号是它的 [`Venue::id`]）。
///
/// # Errors
///
/// 同 [`private_venue`]。
pub fn venue(kind: VenueKind, number: i64) -> Result<Venue, FormatError> {
    Venue::new(PLATFORM, kind, &number.to_string())
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

/// 命令编号 `id` 是哪个机器人号收进来的：[`command_id`] 拼的 `qq:<机器人的号>:…` 的第二段（施工 O-32：桥起来时照日志里最近一条
/// 人话认，「群里怎么叫她」第 1 条）。后面加了 `/decided` 这类的也认。别的平台、不是这样拼的（自己编的）是空的。
pub fn bot_of(id: &str) -> Option<i64> {
    let mut parts = id.split(':');
    if parts.next()? != PLATFORM {
        return None;
    }
    parts.next()?.parse().ok()
}

/// 场所 `venue` 发到哪（施工 O-32）：平台是 [`PLATFORM`] 的，私聊发给对方、群发进群；别的平台、号不是整数的是空的。
pub fn to_of(venue: &Venue) -> Option<To> {
    if venue.platform() != PLATFORM {
        return None;
    }
    let number = venue.number().parse().ok()?;
    Some(match venue.kind() {
        VenueKind::Private => To::Private(number),
        VenueKind::Group => To::Group(number),
    })
}

/// 一条消息：私聊的或者群里的（第 5 条，「群消息」第 1 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Posted {
    /// 收到它的机器人的号（事件的 `self_id`）。
    pub bot: i64,
    /// 发的人的号（`user_id`）。
    pub user: i64,
    /// 消息编号（`message_id`）：拼进命令编号，断线重发、平台重发都只收一次（第 8 条）。
    pub message_id: i64,
    /// 平台给的时刻（事件的 `time`，整数秒）：也拼进命令编号（第 8 条）。没带、读不出的是 0（「施工时定的」第 20 条）：消息
    /// 照送，去重退回只看消息编号。
    pub time: i64,
    /// 发的人此刻叫什么：`sender` 的群名片，空白的取昵称（[`display_name`]，施工 O-22）；都没有的是空的。照原样，没洗。
    pub name: Option<String>,
    /// 发的人在群里的身份：`sender.role`（[`rank_of`]，施工 O-31）；私聊的、没带的、认不出的是空的。
    pub rank: Option<Rank>,
    /// 读出来的字（第 6 条）：只有 `text` 段，照原样，没去掉首尾空白。私聊照它送。
    pub text: String,
    /// 认出来的段（施工 O-22，[`segments()`]）：群的正文照它写；私聊取引用和带的东西。
    pub segments: Segments,
}

/// 一次撤回（「撤回」，施工 O-22）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recall {
    /// 收到它的机器人的号。
    pub bot: i64,
    /// 群号：私聊的撤回是空的。
    pub group: Option<i64>,
    /// 被撤的那一条是谁发的（`user_id`）：私聊里就是对方。
    pub user: i64,
    /// 谁撤的：群里是 `operator_id`，私聊是 `user_id`。
    pub by: i64,
    /// 被撤的那一条的消息编号。
    pub message_id: i64,
}

/// 交给跟核心的那一头的一件事。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// 一条私聊。
    Private(Posted),
    /// 一条群消息（施工 O-22）。
    Group {
        /// 群号（`group_id`）。
        group: i64,
        /// 这条消息。
        posted: Posted,
    },
    /// 一次撤回（施工 O-22）。
    Recalled(Recall),
    /// 她在群里被禁言了（施工 O-25 中，「出站队列」第 7 条）。
    Muted {
        /// 收到通知的机器人的号：被禁言的就是它。
        bot: i64,
        /// 群号。
        group: i64,
        /// 禁几秒，大于 0。
        seconds: u64,
    },
    /// 她在群里被解禁了（施工 O-25 中）：`lift_ban`，或者禁 0 秒。
    Unmuted {
        /// 收到通知的机器人的号。
        bot: i64,
        /// 群号。
        group: i64,
    },
    /// 一个机器人号连上了（施工 O-25 中，「怎么走」第 2 条）：号认出来的那一刻（`X-Self-ID`、第一条事件），由连接那一头交，
    /// 不是 NapCat 的一帧。排着的照先后发。
    Connected {
        /// 机器人的号。
        bot: i64,
    },
    /// 有人要加她好友（施工 O-27，「好友请求」）：`post_type = request`、`request_type = friend`。
    Befriend {
        /// 收到请求的机器人的号：同意时经它的连接回。
        bot: i64,
        /// 要加她的人的号。
        user: i64,
        /// 平台给这次请求的标记：同意时原样交回（`set_friend_add_request` 的 `flag`）。不进运行日志。
        flag: String,
    },
    /// 有人邀请她进群（施工 O-27）：`request_type = group`、`sub_type = invite`。只记一行，放着。
    Invited {
        /// 收到邀请的机器人的号。
        bot: i64,
        /// 邀请她进的群。
        group: i64,
        /// 邀请的人。
        user: i64,
    },
}

/// NapCat 发来的一帧。
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    /// 动作的回应：带 `echo`、不是事件。
    Reply(Value),
    /// 一条消息、一次撤回、禁言和解禁（施工 O-25 中）、好友请求和群邀请（施工 O-27）。
    Event(Event),
    /// 别的事件（别的通知、请求、心跳、生命周期、机器人自己发的……）或者读不懂的：`post_type`，没有的是空字。
    Other(String),
}

/// 认一帧（第 5 条）。
pub fn read(frame: Value) -> Frame {
    let kind = frame["post_type"].as_str().unwrap_or_default().to_string();
    if kind.is_empty() && frame.get("echo").is_some() {
        return Frame::Reply(frame);
    }
    let event = match kind.as_str() {
        "message" => posted(&frame),
        "notice" => recall(&frame).or_else(|| ban(&frame)),
        "request" => request(&frame),
        _ => None,
    };
    match event {
        Some(event) => Frame::Event(event),
        None => Frame::Other(kind),
    }
}

/// 一条消息：私聊、群的，号都带得出、不是机器人自己发的（「施工时定的」第 69 条）才是。
fn posted(frame: &Value) -> Option<Event> {
    let (bot, user, message_id) = (
        number(&frame["self_id"])?,
        number(&frame["user_id"])?,
        number(&frame["message_id"])?,
    );
    if user == bot {
        return None;
    }
    let group = match frame["message_type"].as_str() {
        Some("private") => None,
        Some("group") => Some(number(&frame["group_id"])?),
        _ => return None,
    };
    let posted = Posted {
        bot,
        user,
        message_id,
        time: number(&frame["time"]).unwrap_or(0),
        name: display_name(&frame["sender"]),
        rank: rank_of(&frame["sender"]),
        text: text_of(&frame["message"]),
        segments: segments(&frame["message"]),
    };
    Some(match group {
        Some(group) => Event::Group { group, posted },
        None => Event::Private(posted),
    })
}

/// 一次撤回：群的（`group_recall`）、私聊的（`friend_recall`），号都带得出的才是。
fn recall(frame: &Value) -> Option<Event> {
    let (bot, user, message_id) = (
        number(&frame["self_id"])?,
        number(&frame["user_id"])?,
        number(&frame["message_id"])?,
    );
    let (group, by) = match frame["notice_type"].as_str() {
        Some("group_recall") => (
            Some(number(&frame["group_id"])?),
            number(&frame["operator_id"])?,
        ),
        Some("friend_recall") => (None, user),
        _ => return None,
    };
    Some(Event::Recalled(Recall {
        bot,
        group,
        user,
        by,
        message_id,
    }))
}

/// 禁言、解禁（`group_ban`，施工 O-25 中，「出站队列」第 7 条）：禁的是她（`user_id` 等于 `self_id`）的才是，全员禁言
/// （`user_id` 是 0）、禁别人的不是。`ban` 带的 `duration`（秒）大于 0 是禁言，0 是解禁；`lift_ban` 是解禁；没带 `duration`、
/// 负的、别的 `sub_type` 不是（「施工时定的」第 121 条）。
fn ban(frame: &Value) -> Option<Event> {
    if frame["notice_type"] != "group_ban" {
        return None;
    }
    let (bot, user, group) = (
        number(&frame["self_id"])?,
        number(&frame["user_id"])?,
        number(&frame["group_id"])?,
    );
    if user != bot {
        return None;
    }
    let seconds = match frame["sub_type"].as_str()? {
        "ban" => u64::try_from(number(&frame["duration"])?).ok()?,
        "lift_ban" => 0,
        _ => return None,
    };
    Some(match seconds {
        0 => Event::Unmuted { bot, group },
        seconds => Event::Muted {
            bot,
            group,
            seconds,
        },
    })
}

/// 请求（施工 O-27，「好友请求」）：加好友的（`friend`，带得出号、不空的标记）、邀请她进群的（`group` 里 `sub_type` 是
/// `invite`，带得出号、群号）才是；入群申请（`sub_type` 是 `add`，随入群审批那一步）、别的不是。
fn request(frame: &Value) -> Option<Event> {
    let (bot, user) = (number(&frame["self_id"])?, number(&frame["user_id"])?);
    match frame["request_type"].as_str()? {
        "friend" => {
            let flag = frame["flag"].as_str().filter(|flag| !flag.is_empty())?;
            Some(Event::Befriend {
                bot,
                user,
                flag: flag.to_string(),
            })
        }
        "group" if frame["sub_type"] == "invite" => Some(Event::Invited {
            bot,
            group: number(&frame["group_id"])?,
            user,
        }),
        _ => None,
    }
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

/// 发给谁：私聊的对方、群（第 10 条，「群消息」第 7 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum To {
    /// 和号是它的人的私聊。
    Private(i64),
    /// 群号是它的群（施工 O-22）。
    Group(i64),
}

/// 一段前面带的引用和 @（施工 O-25 上，`onebot.md`「群里怎么叫她」第 9 条）：引用的那一条的平台编号、@ 的号，照平台给的原样
/// 写成字（旧版的教训：引用的编号原样还回去，自作聪明换写法的，对端会不声不响地丢掉引用）。都没有的是不带。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lead {
    /// 引用的那一条的平台编号：`reply` 段。
    pub reply: Option<String>,
    /// @ 的号：`at` 段，后面跟一个空格的文字段。
    pub at: Option<String>,
}

/// 发一句 `text` 给 `to` 的动作和参数：`send_private_msg {user_id, message}` 或者 `send_group_msg {group_id, message}`
/// （第 10 条）。`message` 先是 `lead` 要带的引用段、@ 段，再是一个文字段（施工 O-25 上）。
pub fn message_to(to: To, text: &str, lead: &Lead) -> (&'static str, Value) {
    let mut message = Vec::new();
    if let Some(id) = &lead.reply {
        message.push(json!({"type": "reply", "data": {"id": id}}));
    }
    if let Some(qq) = &lead.at {
        message.push(json!({"type": "at", "data": {"qq": qq}}));
        // @ 段和后面的字挨着画，隔一个空格好读（旧版这样发，客户端不一定自己隔开）。
        message.push(json!({"type": "text", "data": {"text": " "}}));
    }
    message.push(json!({"type": "text", "data": {"text": text}}));
    match to {
        To::Private(user) => (
            "send_private_msg",
            json!({"user_id": user, "message": message}),
        ),
        To::Group(group) => (
            "send_group_msg",
            json!({"group_id": group, "message": message}),
        ),
    }
}

/// 撤回平台编号是 `message_id` 的那一条的动作和参数：`delete_msg {message_id}`（施工 O-25 上，「斜杠命令」第 7 条）。编号是
/// NapCat 回的整数（回执），或者引用里原样的字（平台工具，施工 O-31：原样还回去，同 [`emoji_like`]）；NapCat 两样都收
/// （`DeleteMsg.ts`）。
pub fn delete_msg(message_id: impl Into<Value>) -> (&'static str, Value) {
    ("delete_msg", json!({"message_id": message_id.into()}))
}

/// 在群 `group` 里禁言号是 `user` 的人 `seconds` 秒的动作和参数（0 是解禁）：`set_group_ban {group_id, user_id, duration}`（施工
/// O-31，「平台工具（一）」第 1 条）。
pub fn group_ban(group: i64, user: i64, seconds: u64) -> (&'static str, Value) {
    (
        "set_group_ban",
        json!({"group_id": group, "user_id": user, "duration": seconds}),
    )
}

/// 戳一戳号是 `user` 的人的动作和参数（施工 O-31）：在 `to` 那个群里 `group_poke {group_id, user_id}`，私聊里
/// `friend_poke {user_id}`（NapCat 的 `SendPoke.ts`：群号没写的是私聊）。
pub fn poke(to: To, user: i64) -> (&'static str, Value) {
    match to {
        To::Group(group) => ("group_poke", json!({"group_id": group, "user_id": user})),
        To::Private(_) => ("friend_poke", json!({"user_id": user})),
    }
}

/// 在平台编号是 `message` 的那一条上贴（`set` 是真）、摘（假）表情 `emoji` 的动作和参数：`set_msg_emoji_like {message_id,
/// emoji_id, set}`（施工 O-25 下，「贴表情」第 2 条）。编号、表情都原样写成字：NapCat 数和字都收（`SetMsgEmojiLike.ts`），
/// 编号原样还回去，不换写法（「施工时定的」第 109、132 条）。
pub fn emoji_like(message: &str, emoji: &str, set: bool) -> (&'static str, Value) {
    (
        "set_msg_emoji_like",
        json!({"message_id": message, "emoji_id": emoji, "set": set}),
    )
}

/// 同意标记是 `flag` 的那次好友请求的动作和参数：`set_friend_add_request {flag, approve: true}`（施工 O-27，「好友请求」）。
/// 标记照平台给的原样交回。
pub fn friend_add(flag: &str) -> (&'static str, Value) {
    (
        "set_friend_add_request",
        json!({"flag": flag, "approve": true}),
    )
}

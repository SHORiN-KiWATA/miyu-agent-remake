//! 群消息（`onebot.md` 第一条「群消息」，施工 O-22）：群里的每一条消息都记进这个群的场所会话，一律旁听，交的时候不开回合
//! （18 第五节）；记下了交给 `called` 判要不要叫她（施工 O-23，「群里怎么叫她」）。
//!
//! 1. 场所、平台上的人经群聊内核拼（`onebot::venue`、`onebot::person`），拼不出的记一行、这条不送。
//! 2. 场所规则套到这个群上（`applied`）：找会话带的人格、预设、工作区，发的人是不是管理的人，睡没睡，看不看得到号。
//! 3. 发的人的名字、身份（施工 O-31）顺手记进群成员的缓存；正文里的 @ 写成名字（`names`）。
//! 4. 正文空白、又没有带的东西的不送；只有带的东西的照样交（「施工时定的」第 67 条）。
//! 5. 照私聊的办法交（`Route::submit`）：`/` 开头的先当斜杠命令，回执发回群里；别的 `session.send` 带场所的格。
//! 6. 核心记下了（回应交回序号）的交给 `called` 判（施工 O-23）。

use std::time::Instant;

use miyu_chat::VenueKind;
use serde_json::json;

use super::called::Heard;
use super::fields::{Flags, fields};
use super::session::Place;
use super::{Message, Route, applied};
use crate::TARGET;
use crate::core::Gone;
use crate::onebot::{Posted, command_id, person, venue};

impl Route {
    /// 群号是 `group` 的群里的一条消息 `posted`（「群消息」第 3 到 7 条）。
    pub(super) async fn group(&mut self, group: i64, posted: Posted) -> Result<(), Gone> {
        let made =
            venue(VenueKind::Group, group).and_then(|venue| Ok((venue, person(posted.user)?)));
        let (venue, external) = match made {
            Ok(made) => made,
            Err(error) => {
                tracing::warn!(target: TARGET, group, user = posted.user, message = posted.message_id, error = %error, "venue not made, message dropped");
                return Ok(());
            }
        };
        let applied = self.applied(&venue);
        if let Some(name) = &posted.name {
            self.members
                .remember(group, posted.user, name.clone(), Instant::now());
        }
        if let Some(rank) = posted.rank {
            // 禁言不动群主、群管理员（施工 O-31，「平台工具（一）」第 7 条）。
            self.members
                .ranked(group, posted.user, rank, Instant::now());
        }
        let text = self.named(group, &posted).await;
        if text.trim().is_empty() && posted.segments.media.is_empty() {
            tracing::debug!(target: TARGET, venue = %venue.id(), message = posted.message_id, "nothing to send");
            return Ok(());
        }
        let at = posted.segments.at();
        // @ 了的别人；拼不出平台身份的（号是整数，照说不会）不记。
        let mentions: Vec<_> = at
            .iter()
            .filter(|user| **user != posted.bot)
            .filter_map(|user| person(*user).ok())
            .collect();
        let flags = Flags {
            mentions_me: at.contains(&posted.bot),
            ambient: true,
            asleep: applied::asleep(&applied),
            show_ids: applied::show_ids(&applied),
        };
        let message = Message {
            id: command_id(posted.bot, posted.message_id, posted.time),
            number: posted.message_id,
            acting: json!({"external": external, "role": applied::role(&applied, &external)}),
            fields: fields(&posted, &mentions, flags),
            place: Place::group(&venue, &applied, posted.bot, group),
            text,
        };
        let heard = Heard {
            id: message.id.clone(),
            number: message.number,
            venue,
            sender: external,
            text: message.text.clone(),
            mentions_me: flags.mentions_me,
            mentions_others: !mentions.is_empty(),
            reply_to: posted.segments.reply_to.clone(),
            media: posted
                .segments
                .media
                .iter()
                .map(|media| media.kind)
                .collect(),
        };
        let Some((session, reply)) = self.submit(message).await? else {
            return Ok(());
        };
        // 被拒的没有序号，不判。
        match reply["result"]["events"][0].as_u64() {
            Some(seq) => self.called(&session, seq, heard).await,
            None => Ok(()),
        }
    }
}

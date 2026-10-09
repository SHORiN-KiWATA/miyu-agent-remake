//! 她被禁言、解禁（施工 O-25 中，`onebot.md` 第一条「出站队列」第 7 条）：NapCat 推来的 `group_ban`（禁的是她的）记进这个群
//! 的会话：禁言记 `ext.onebot.venues.muted {until}`（`until` 是本机此刻加禁言的秒数，「施工时定的」第 122 条），解禁记
//! `ext.onebot.venues.unmuted`。记下以后把留着的推送收进投影（进站链的 `Ctx.muted`、出站的门都照投影），再看一遍排着的：解禁了
//! 的照先后发，禁言改短了的照新的到期时刻醒。会话照群消息一样找（`session`）；命令编号自己编：平台不重发通知（同撤回）。

use miyu_chat::VenueKind;
use serde_json::json;

use super::Route;
use super::applied;
use super::projection::{MUTED, UNMUTED};
use super::queue::until;
use super::session::Place;
use crate::TARGET;
use crate::core::{Gone, reason};
use crate::onebot::venue;

impl Route {
    /// 机器人号 `bot` 在群号是 `group` 的群里被禁言 `seconds` 秒；空的是解禁了。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn muted(
        &mut self,
        bot: i64,
        group: i64,
        seconds: Option<u64>,
    ) -> Result<(), Gone> {
        let venue = match venue(VenueKind::Group, group) {
            Ok(venue) => venue,
            Err(error) => {
                tracing::warn!(target: TARGET, group, error = %error, "venue not made, mute not noted");
                return Ok(());
            }
        };
        let (kind, body) = match seconds {
            Some(seconds) => {
                let Some(end) = applied::clock().and_then(|clock| until(clock.now, seconds)) else {
                    tracing::warn!(target: TARGET, venue = %venue.id(), seconds, "mute not noted");
                    return Ok(());
                };
                (MUTED, json!({"until": end}))
            }
            None => (UNMUTED, json!({})),
        };
        let place = Place::group(&venue, &self.applied(&venue), bot, group);
        let params = json!({"kind": kind, "body": body});
        let Some((session, reply)) = self
            .on_session(&place, None, "events.append", params)
            .await?
        else {
            return Ok(());
        };
        match (reason(&reply), seconds) {
            (None, Some(seconds)) => {
                tracing::info!(target: TARGET, venue = %venue.id(), seconds, "mute noted");
            }
            (None, None) => tracing::info!(target: TARGET, venue = %venue.id(), "unmute noted"),
            (Some(reason), _) => {
                tracing::warn!(target: TARGET, venue = %venue.id(), reason, "mute refused");
            }
        }
        self.catch_up(&session).await?;
        self.pump().await
    }
}

//! 撤回（`onebot.md` 第一条「撤回」，施工 O-22）：群里的（`group_recall`）、私聊的（`friend_recall`）都记
//! `events.append {session, kind: "venue.recalled", body: {msg, by}}`：被撤的平台编号、谁撤的平台身份。会话照群消息、私聊
//! 一样找（私聊的陌生人照旧不接，白名单成员的照接，施工 O-27）；会话不在了再找一次。命令编号自己编：平台不重发通知
//! （「施工时定的」第 70 条）。

use miyu_chat::VenueKind;
use serde_json::json;

use super::Route;
use super::session::Place;
use crate::TARGET;
use crate::core::{Gone, reason};
use crate::onebot::{Recall, person, venue};

impl Route {
    /// 一次撤回 `recall`。
    pub(super) async fn recalled(&mut self, recall: Recall) -> Result<(), Gone> {
        let (kind, number) = match recall.group {
            Some(group) => (VenueKind::Group, group),
            None => (VenueKind::Private, recall.user),
        };
        let made = venue(kind, number)
            .and_then(|venue| Ok((venue, person(recall.user)?, person(recall.by)?)));
        let (venue, user, by) = match made {
            Ok(made) => made,
            Err(error) => {
                tracing::warn!(target: TARGET, user = recall.user, message = recall.message_id, error = %error, "venue not made, recall dropped");
                return Ok(());
            }
        };
        let applied = self.applied(&venue);
        let place = match recall.group {
            Some(group) => Place::group(&venue, &applied, recall.bot, group),
            None => {
                let listed = self.whitelisted(&user).then_some(&applied);
                Place::private(&venue, &user, (recall.bot, recall.user), listed)
            }
        };
        let body = json!({"msg": recall.message_id.to_string(), "by": by});
        let params = json!({"kind": "venue.recalled", "body": body});
        let Some((_, reply)) = self
            .on_session(&place, None, "events.append", params)
            .await?
        else {
            return Ok(());
        };
        let message = recall.message_id;
        match reason(&reply) {
            None => tracing::info!(target: TARGET, venue = %venue.id(), message, "recall noted"),
            Some(reason) => {
                tracing::warn!(target: TARGET, venue = %venue.id(), message, reason, "recall refused");
            }
        }
        Ok(())
    }
}

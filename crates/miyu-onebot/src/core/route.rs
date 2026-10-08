//! 私聊进来、回话出去（`onebot.md` 第一条「怎么走」第 6 到 10 条）：一个任务拿着跟核心的连接，一边收 NapCat 那头读出来的
//! 私聊，一边收核心推来的事件。私聊一条条照先后办（找会话、订阅、发进去），办的时候来的推送由 [`Core`] 留着、办完再看。
//!
//! - 场所、平台上的人经群聊内核拼（`onebot::private_venue`、`onebot::person`），拼不出来的（照说不会）记一行、这条不送。
//! - 「场所 → 会话编号」只记在内存里：每个场所桥起来以后第一次来消息时问一次 `venue.session`，问到了订阅它（不写 `after`）。
//! - 会话不在了（`session_not_found`、`session_stopped`）：忘掉，再问一次、再发一次，只重来一次。
//! - 不是主人（`no_system_account`）：这一步不接，同一个人只记一行运行日志。
//! - `/` 开头的先当斜杠命令交 `command.run`（`command`，O-19，第 7、8 条之间的「斜杠命令」），核心说不是命令的才照普通的话发。
//! - 她的回话：`message.assistant` 的文字块接起来，不空就交给收进这个会话的那个机器人号现在的连接；`begin` 在这里照先后放进
//!   写队列，等回应的那一步交给别的任务。
//! - 核心推来的 `extension.config`（施工 O-20）：`keys` 原样交给 `serve`，它换上手里的配置、端口变了照 `/apply` 的办法换
//!   （「施工时定的」第 45 条）：这里够不着监听。

mod command;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use miyu_kernel::FormatError;
use miyu_kernel::id::{ExternalId, VenueId};
use serde_json::{Map, Value, json};
use tokio::sync::mpsc;
use tokio::task::{JoinError, JoinSet};

use crate::TARGET;
use crate::core::{Core, Gone, reason};
use crate::listen::bots::Bots;
use crate::onebot::{Private, command_id, person, private_message, private_venue};
use crate::serve::Failure;

/// 一个场所会话的回话发到哪：收进它的机器人号、对方的号；场所只拿来记运行日志。
#[derive(Debug, Clone)]
struct Peer {
    bot: i64,
    user: i64,
    venue: VenueId,
}

/// 一条要交给核心的私聊：命令编号（第 8 条）、场所和对方在平台上的身份（第 7 条），和私聊本身。
struct Message {
    id: String,
    venue: VenueId,
    external: ExternalId,
    private: Private,
}

/// 拿着跟核心的连接的那一个。
pub(crate) struct Route {
    core: Core,
    bots: Arc<Bots>,
    /// 场所 → 会话编号（第 7 条）。键是场所编号的原文：内核的 `VenueId` 不能做散列表的键。
    venues: HashMap<String, String>,
    /// 会话编号 → 回话发到哪（第 10 条）。
    peers: HashMap<String, Peer>,
    /// 记过一行「不是主人」的号（第 7 条）。
    refused: HashSet<i64>,
    /// 在等 NapCat 回应的回话：放下 `Route` 时一起停。
    sending: JoinSet<()>,
    /// 推来的配置变化交给 `serve`（施工 O-20）。
    configured: mpsc::UnboundedSender<Map<String, Value>>,
}

impl Route {
    /// 拿着连接 `core`，回话照 `bots` 找连接，推来的配置变化交给 `configured`。
    pub(crate) fn new(
        core: Core,
        bots: Arc<Bots>,
        configured: mpsc::UnboundedSender<Map<String, Value>>,
    ) -> Route {
        Route {
            core,
            bots,
            venues: HashMap::new(),
            peers: HashMap::new(),
            refused: HashSet::new(),
            sending: JoinSet::new(),
            configured,
        }
    }

    /// 一直办，直到办不下去：核心关了管道、写不出去（核心请它退出、核心不在了，第 11 条），交回空的，桥好好停下；发回话
    /// 的任务崩了，交回 [`Failure::Crashed`]（「施工时定的」第 14 条），桥照它退出。`inbound` 关了就只看核心：桥在停、
    /// 读 NapCat 的那一头都放下了。
    pub(crate) async fn run(mut self, mut inbound: mpsc::Receiver<Private>) -> Option<Failure> {
        let mut open = true;
        loop {
            let handled = tokio::select! {
                private = inbound.recv(), if open => match private {
                    Some(private) => self.private(private).await.map_err(|Gone| None),
                    None => {
                        open = false;
                        Ok(())
                    }
                },
                pushed = self.core.next() => match pushed {
                    Some(pushed) => self.pushed(pushed).await.map_err(|Gone| None),
                    None => Err(None),
                },
                Some(joined) = self.sending.join_next(), if !self.sending.is_empty() => sent(joined).map_err(Some),
            };
            if let Err(ended) = handled {
                return ended;
            }
        }
    }

    /// 一条私聊（第 6 到 8 条，中间是「斜杠命令」）。
    async fn private(&mut self, private: Private) -> Result<(), Gone> {
        let (venue, external) = match addressed(private.user) {
            Ok(addressed) => addressed,
            Err(error) => {
                tracing::warn!(target: TARGET, user = private.user, message = private.message_id, error = %error, "venue not made, message dropped");
                return Ok(());
            }
        };
        if private.text.trim().is_empty() {
            tracing::debug!(target: TARGET, venue = %venue, message = private.message_id, "nothing to send");
            return Ok(());
        }
        let message = Message {
            id: command_id(private.bot, private.message_id, private.time),
            venue,
            external,
            private,
        };
        if command::looks_like(&message.private.text) && self.command(&message).await? {
            return Ok(());
        }
        let Some((_, reply)) = self.deliver(&message, "session.send").await? else {
            return Ok(());
        };
        let (venue, number) = (&message.venue, message.private.message_id);
        match reason(&reply) {
            None => {
                let chars = message.private.text.chars().count();
                tracing::info!(target: TARGET, venue = %venue, message = number, chars, "message sent in");
            }
            Some(other) => {
                tracing::warn!(target: TARGET, venue = %venue, message = number, reason = other, "message refused");
            }
        }
        Ok(())
    }

    /// 把私聊 `message` 照 `method`（`session.send`、`command.run`）交给它那个场所的会话：`{session, text, as}`，编号是
    /// 这条消息的命令编号（第 8 条）。会话不在了（`session_not_found`、`session_stopped`）忘掉、再找、再交，只重来一次。
    /// 交回会话编号和最后一次的回应（接受的、拒绝的都原样），这时记下这个会话的回话发给谁（「施工时定的」第 36 条）；
    /// 不接的、找不到会话的是空的（已经记了运行日志）。
    async fn deliver(
        &mut self,
        message: &Message,
        method: &str,
    ) -> Result<Option<(String, Value)>, Gone> {
        let Message {
            id,
            venue,
            external,
            private,
        } = message;
        for retried in [false, true] {
            let session = match self.venues.get(venue.as_str()) {
                Some(session) => session.clone(),
                None => match self.find(venue, external, private.user).await? {
                    Some(session) => session,
                    None => return Ok(None),
                },
            };
            let params = json!({
                "session": session,
                "text": private.text,
                "as": {"external": external},
            });
            let reply = self.core.call_as(id, method, params).await?;
            if !retried
                && matches!(
                    reason(&reply),
                    Some("session_not_found" | "session_stopped")
                )
            {
                self.venues.remove(venue.as_str());
                continue;
            }
            let peer = Peer {
                bot: private.bot,
                user: private.user,
                venue: venue.clone(),
            };
            self.peers.insert(session.clone(), peer);
            return Ok(Some((session, reply)));
        }
        Ok(None)
    }

    /// 找回场所 `venue` 的会话、订阅它（第 7、9 条）：对方是平台上的 `peer`，号是 `user`。不接的、问不到的、订阅不上的是
    /// 空的（记一行运行日志）。
    async fn find(
        &mut self,
        venue: &VenueId,
        peer: &ExternalId,
        user: i64,
    ) -> Result<Option<String>, Gone> {
        let params = json!({"venue": venue, "kind": "private", "peer": peer});
        let reply = self.core.call("venue.session", params).await?;
        match reason(&reply) {
            None => {}
            Some("no_system_account") => {
                if self.refused.insert(user) {
                    tracing::info!(target: TARGET, venue = %venue, "not the owner, not taken");
                }
                return Ok(None);
            }
            Some(other) => {
                tracing::warn!(target: TARGET, venue = %venue, reason = other, "venue session not found");
                return Ok(None);
            }
        }
        let Some(session) = reply["result"]["session"].as_str().map(str::to_string) else {
            tracing::warn!(target: TARGET, venue = %venue, "venue session without an id");
            return Ok(None);
        };
        if !self.subscribe(&session).await? {
            return Ok(None);
        }
        self.venues.insert(venue.to_string(), session.clone());
        Ok(Some(session))
    }

    /// 订阅会话 `session` 的事件流，不写 `after`（第 9 条）：只要以后的新事件。订阅不上的记一行、交回假。
    async fn subscribe(&mut self, session: &str) -> Result<bool, Gone> {
        let params = json!({"session": session, "stream": "events"});
        let reply = self.core.call("subscribe", params).await?;
        if let Some(reason) = reason(&reply) {
            tracing::warn!(target: TARGET, session, reason, "not subscribed");
            return Ok(false);
        }
        Ok(true)
    }

    /// 核心推来的一条：配置变了的（`extension.config`）交给 `serve`（施工 O-20）；她的回话发回去（第 10 条）；掉了队、会话停了的
    /// （`resync`）再订阅一次。
    async fn pushed(&mut self, pushed: Value) -> Result<(), Gone> {
        let params = &pushed["params"];
        if pushed["method"] == "extension.config" {
            if let Some(keys) = params["keys"].as_object()
                && self.configured.send(keys.clone()).is_err()
            {
                // `serve` 不收了：桥在停，没有别处可交。
            }
            return Ok(());
        }
        let Some(session) = params["session"].as_str() else {
            return Ok(());
        };
        match pushed["method"].as_str() {
            Some("event") if params["event"]["kind"] == "message.assistant" => {
                let text = reply_text(&params["event"]);
                self.send_back(session, &text).await;
            }
            Some("resync") if params["stream"] == "events" => {
                let session = session.to_string();
                self.subscribe(&session).await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// 把回话 `text` 发回会话 `session` 的那个私聊：空的不发；那个号没连着的记一行、丢掉。
    async fn send_back(&mut self, session: &str, text: &str) {
        let Some(Peer { bot, user, venue }) = self.peers.get(session).cloned() else {
            return;
        };
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let chars = text.chars().count();
        let Some(link) = self.bots.get(bot) else {
            tracing::info!(target: TARGET, venue = %venue, bot, chars, "bot not connected, reply dropped");
            return;
        };
        let begun = link
            .calls
            .begin(&link.out, "send_private_msg", private_message(user, text))
            .await;
        let pending = match begun {
            Ok(pending) => pending,
            Err(error) => {
                tracing::warn!(target: TARGET, venue = %venue, chars, error = ?error, "reply not sent");
                return;
            }
        };
        self.sending.spawn(async move {
            match pending.wait().await {
                Ok(_) => tracing::info!(target: TARGET, venue = %venue, chars, "reply sent"),
                Err(error) => {
                    tracing::warn!(target: TARGET, venue = %venue, chars, error = ?error, "reply not sent");
                }
            }
        });
    }
}

/// 和号 `user` 的私聊的场所编号、对方在平台上的身份（第 7 条），都经群聊内核拼。
fn addressed(user: i64) -> Result<(VenueId, ExternalId), FormatError> {
    Ok((private_venue(user)?, person(user)?))
}

/// 一个发回话的任务结束了。崩了（是 bug：任务里只等回应、记日志）照实交上去，桥照「施工时定的」第 14 条停下，不装作
/// 还在跑；记一行运行日志。任务不会被掐掉：掐它们的只有放下 `Route`，那时已经不在这里等了。
fn sent(joined: Result<(), JoinError>) -> Result<(), Failure> {
    joined.map_err(|error| {
        tracing::error!(target: TARGET, error = %error, "reply task crashed");
        Failure::Crashed(error.to_string())
    })
}

/// 她一条回话里的文字：`text` 块依次接起来，思考、工具调用不要（第 10 条）。
fn reply_text(event: &Value) -> String {
    event["body"]["blocks"]
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|block| block["type"] == "text")
                .filter_map(|block| block["text"].as_str())
                .collect()
        })
        .unwrap_or_default()
}

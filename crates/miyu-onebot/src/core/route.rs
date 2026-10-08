//! 消息进来、回话出去（`onebot.md` 第一条「怎么走」第 6 到 10 条，「群消息」「撤回」）：一个任务拿着跟核心的连接，一边收
//! NapCat 那头读出来的消息和撤回，一边收核心推来的事件。一件件照先后办（套场所规则、找会话、发进去），办的时候来的推送由
//! [`Core`] 留着、办完再看。
//!
//! - 私聊（这里）：场所、平台上的人经群聊内核拼（`onebot::venue`、`onebot::person`），拼不出来的（照说不会）记一行、这条
//!   不送；带上场所的格（施工 O-22，`fields`），`show_ids` 照这个私聊套出来的场所规则（`applied`）。
//! - 群消息（`group`，施工 O-22）：一律旁听；正文里的 @ 写成名字（`names`）；场所规则管人格、预设、工作区、谁是管理的人、
//!   睡没睡、看不看得到号。撤回（`recall`，施工 O-22）记 `venue.recalled`。
//! - 找会话（`session`）：「场所 → 会话编号」只记在内存里，每个场所桥起来以后第一次要用时问一次 `venue.session`；私聊的
//!   问到了订阅（不写 `after`），群的不订阅（「施工时定的」第 64 条）。会话不在了（`session_not_found`、`session_stopped`）
//!   忘掉，再问一次、再交一次，只重来一次。私聊不是主人的（`no_system_account`，或者会话的属主是桥自己，「施工时定的」第
//!   49 条）、群的规则写错的不接，同一个场所只记一行运行日志。
//! - `/` 开头的先当斜杠命令交 `command.run`（`command`，O-19，第 7、8 条之间的「斜杠命令」），核心说不是命令的才照普通的话发。
//! - 她的回话：`message.assistant` 的文字块接起来，不空就交给收进这个会话的那个机器人号现在的连接；`begin` 在这里照先后放进
//!   写队列，等回应的那一步交给别的任务。
//! - 核心推来的 `extension.config`（施工 O-20）：`keys` 原样交给 `serve`，它换上手里的配置、端口变了照 `/apply` 的办法换
//!   （「施工时定的」第 45 条）：这里够不着监听。

mod applied;
mod command;
mod fields;
mod group;
mod names;
mod recall;
mod session;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use miyu_chat::{Venue, VenueKind};
use miyu_kernel::id::VenueId;
use serde_json::{Map, Value, json};
use tokio::sync::mpsc;
use tokio::task::{JoinError, JoinSet};

use crate::TARGET;
use crate::core::{Core, Gone, reason};
use crate::listen::bots::Bots;
use crate::onebot::{Event, Members, Posted, To, command_id, message_to, person, venue};
use crate::rules::{Applied, Venues};
use crate::serve::Failure;
use fields::{Flags, fields};
use session::Place;

/// 一个场所会话的回执、回话发到哪：收进它的机器人号、私聊的对方或者群；场所只拿来记运行日志。
#[derive(Debug, Clone)]
struct Peer {
    bot: i64,
    to: To,
    venue: VenueId,
}

/// 一条要交给核心的消息（私聊的、群的）。
struct Message {
    /// 命令编号（第 8 条）。
    id: String,
    /// 平台的消息编号：只记运行日志。
    number: i64,
    /// 交给核心的正文。
    text: String,
    /// `as`：发的人的平台身份，群里另带 `role`（「群消息」第 4 条）。
    acting: Value,
    /// `session.send` 的 `venue`（施工 O-22，`fields`）。
    fields: Value,
    /// 它的场所：怎么找会话、回执发到哪。
    place: Place,
}

/// 拿着跟核心的连接的那一个。
pub(crate) struct Route {
    core: Core,
    bots: Arc<Bots>,
    /// 场所规则和出厂数据（施工 O-21）：每一条消息照它套场所（施工 O-22）。
    rules: Venues,
    /// 群成员的名字（施工 O-22，「群消息」第 5 条）。
    members: Members,
    /// 场所 → 会话编号（第 7 条）。键是场所编号的原文：内核的 `VenueId` 不能做散列表的键。
    venues: HashMap<String, String>,
    /// 会话编号 → 回执、回话发到哪（第 10 条）。
    peers: HashMap<String, Peer>,
    /// 记过一行「不接」的场所（第 7 条、「群消息」第 3 条）：私聊不是主人的，群的规则写错的。
    refused: HashSet<String>,
    /// 在等 NapCat 回应的回话：放下 `Route` 时一起停。
    sending: JoinSet<()>,
    /// 推来的配置变化交给 `serve`（施工 O-20）。
    configured: mpsc::UnboundedSender<Map<String, Value>>,
}

impl Route {
    /// 拿着连接 `core`，回话照 `bots` 找连接，场所规则照 `rules`，群成员的名字记进 `members`，推来的配置变化交给
    /// `configured`。
    pub(crate) fn new(
        core: Core,
        bots: Arc<Bots>,
        rules: Venues,
        members: Members,
        configured: mpsc::UnboundedSender<Map<String, Value>>,
    ) -> Route {
        Route {
            core,
            bots,
            rules,
            members,
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
    pub(crate) async fn run(mut self, mut inbound: mpsc::Receiver<Event>) -> Option<Failure> {
        let mut open = true;
        loop {
            let handled = tokio::select! {
                event = inbound.recv(), if open => match event {
                    Some(event) => self.event(event).await.map_err(|Gone| None),
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

    /// NapCat 那头读出来的一件事。
    async fn event(&mut self, event: Event) -> Result<(), Gone> {
        match event {
            Event::Private(posted) => self.private(posted).await,
            Event::Group { group, posted } => self.group(group, posted).await,
            Event::Recalled(recall) => self.recalled(recall).await,
        }
    }

    /// 一条私聊（第 6 到 8 条，中间是「斜杠命令」）。
    async fn private(&mut self, posted: Posted) -> Result<(), Gone> {
        let made = venue(VenueKind::Private, posted.user)
            .and_then(|venue| Ok((venue, person(posted.user)?)));
        let (venue, external) = match made {
            Ok(made) => made,
            Err(error) => {
                tracing::warn!(target: TARGET, user = posted.user, message = posted.message_id, error = %error, "venue not made, message dropped");
                return Ok(());
            }
        };
        if posted.text.trim().is_empty() {
            tracing::debug!(target: TARGET, venue = %venue.id(), message = posted.message_id, "nothing to send");
            return Ok(());
        }
        // 私聊每一条都开回合：不写 `ambient`、`asleep`（「施工时定的」第 68 条）。
        let flags = Flags {
            show_ids: applied::show_ids(&self.applied(&venue)),
            ..Flags::default()
        };
        let message = Message {
            id: command_id(posted.bot, posted.message_id, posted.time),
            number: posted.message_id,
            text: posted.text.clone(),
            acting: json!({"external": external}),
            fields: fields(&posted, &[], flags),
            place: Place::private(&venue, &external, posted.bot, posted.user),
        };
        self.submit(message).await
    }

    /// 交一条消息：`/` 开头的先当斜杠命令交（「斜杠命令」），办完了的不再发；别的照 `session.send` 发（第 8 条，「群消息」
    /// 第 6 条）。
    async fn submit(&mut self, message: Message) -> Result<(), Gone> {
        if command::looks_like(&message.text) && self.command(&message).await? {
            return Ok(());
        }
        let Some((_, reply)) = self.deliver(&message, session::SEND).await? else {
            return Ok(());
        };
        let (venue, number) = (&message.place.peer.venue, message.number);
        match reason(&reply) {
            None => {
                let chars = message.text.chars().count();
                tracing::info!(target: TARGET, venue = %venue, message = number, chars, "message sent in");
            }
            Some(other) => {
                tracing::warn!(target: TARGET, venue = %venue, message = number, reason = other, "message refused");
            }
        }
        Ok(())
    }

    /// 这一刻套到场所 `venue` 上的场所规则（「场所规则和出厂数据」第 3、5 条）。
    fn applied(&mut self, venue: &Venue) -> Applied {
        self.rules.current(Instant::now()).at(venue)
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

    /// 把回话、回执 `text` 发回会话 `session` 的那个私聊、群：空的不发；那个号没连着的记一行、丢掉。
    async fn send_back(&mut self, session: &str, text: &str) {
        let Some(Peer { bot, to, venue }) = self.peers.get(session).cloned() else {
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
        let (action, params) = message_to(to, text);
        let begun = link.calls.begin(&link.out, action, params).await;
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

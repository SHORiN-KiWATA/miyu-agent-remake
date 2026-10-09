//! 消息进来、回话出去（`onebot.md` 第一条「怎么走」第 6 到 10 条，「群消息」「撤回」）：一个任务拿着跟核心的连接，一边收
//! NapCat 那头读出来的消息和撤回，一边收核心推来的事件。一件件照先后办（套场所规则、找会话、发进去），办的时候来的推送由
//! [`Core`] 留着、办完再看。
//!
//! - 私聊（这里）：场所、平台上的人经群聊内核拼（`onebot::venue`、`onebot::person`），拼不出来的（照说不会）记一行、这条
//!   不送；带上场所的格（施工 O-22，`fields`），`show_ids` 照这个私聊套出来的场所规则（`applied`）。
//! - 群消息（`group`，施工 O-22）：一律旁听；正文里的 @ 写成名字（`names`）；场所规则管人格、预设、工作区、谁是管理的人、
//!   睡没睡、看不看得到号。撤回（`recall`，施工 O-22）记 `venue.recalled`。
//! - 群里叫她（施工 O-23，「群里怎么叫她」）：记下以后判（`called`：纯逻辑的判断在 `decide`，线路规程在 `discipline`，
//!   判断的样子在 `body`），该回的开一轮；群会话推来的事件收进投影（`projection`），她新说的话过出站链（`outbound`，施工
//!   O-25 上）发回群里（`speak`）。要问判官的（O-23 下）交给 `judges`，任务（`ask`）经并着发的调用口
//!   问，跟核心的那一头接着办别的；判官回来了照号收回来、算分、记判断（`judged`）。判官带这个群会话所用的人格的说明（O-23
//!   补，`persona`）。
//! - 找会话（`session`）：「场所 → 会话编号」只记在内存里，每个场所桥起来以后第一次要用时问一次 `venue.session`；私聊的
//!   问到了订阅（不写 `after`），群的从头订阅（施工 O-23）。会话不在了（`session_not_found`、`session_stopped`）
//!   忘掉，再问一次、再交一次，只重来一次。私聊不是主人的（`no_system_account`，或者会话的属主是桥自己，「施工时定的」第
//!   49 条）、群的规则写错的不接，同一个场所只记一行运行日志。
//! - `/` 开头的先当斜杠命令交 `command.run`（`command`，O-19，第 7、8 条之间的「斜杠命令」），核心说不是命令的才照普通的话发；
//!   回执交 `receipt`，群里的过几秒撤回（施工 O-25 上）。
//! - 她在私聊里的回话：`message.assistant` 的文字块接起来，过出站链、拆段（`speak`，施工 O-25 上）。
//! - 出站队列（施工 O-25 中，「出站队列」）：她的话、提示、回执都先入队（记 `ext.onebot.venues.queued`）再照先后交给收进这个会话
//!   的那个机器人号现在的连接，等回应的那一步交给别的任务，结局照放进写队列的先后交回来记（`sending`；排着、过期的纯逻辑在
//!   `queue`）。她被禁言、解禁的通知记下（`muted`），禁言时这个群的出站排着；号连上了、定时醒了再看一遍排着的。
//! - 核心推来的 `extension.config`（施工 O-20）：`keys` 原样交给 `serve`，它换上手里的配置、端口变了照 `/apply` 的办法换
//!   （「施工时定的」第 45 条）：这里够不着监听。自己人 `onebot.trusted` 这里记一份（施工 O-23）。

mod applied;
mod ask;
mod body;
mod called;
mod command;
mod decide;
mod discipline;
mod fields;
mod group;
mod judged;
mod judges;
mod muted;
mod names;
mod outbound;
mod persona;
mod projection;
mod queue;
mod recall;
mod receipt;
mod sending;
mod session;
mod speak;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use futures_util::stream::FuturesOrdered;
use miyu_chat::{Venue, VenueKind};
use miyu_kernel::id::VenueId;
use serde_json::{Map, Value, json};
use tokio::sync::mpsc;
use tokio::task::{JoinError, JoinHandle, JoinSet};

use crate::TARGET;
use crate::core::{Core, Gone, reason};
use crate::listen::bots::Bots;
use crate::onebot::{Event, Members, Posted, To, command_id, person, venue};
use crate::rules::{Applied, Venues};
use crate::serve::Failure;
use crate::settings::{trusted, trusted_key};
use crate::texts::Texts;
pub(crate) use ask::Slots;
use fields::{Flags, fields};
use judges::Judges;
use outbound::Spoken;
pub(crate) use persona::Personas;
use projection::Projection;
use queue::Queue;
use sending::{Answered, Item};
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
    /// 群会话编号 → 这个群的投影（施工 O-23，「群里怎么叫她」第 1、2 条）：订阅了的群才有。
    groups: HashMap<String, Projection>,
    /// 自己人的平台身份（`onebot.trusted`，施工 O-23）：握手交来的，推来新的就换。
    trusted: Vec<String>,
    /// 发进群里的提示照它说（施工 O-23）：握手回的语言。
    texts: Texts,
    /// 出站排着的（施工 O-25 中，「出站队列」）：门关着（她被禁言、号没连着）的照会话排着，过了期作废。
    waiting: Queue<Item>,
    /// 在等 NapCat 回应的一段段（她的话、提示、回执）：一段一个任务，结果照放进写队列的先后交出来（NapCat 并着办动作，回的
    /// 先后不一定照发的先后；`venue.delivered` 要照发的先后记，核心照日志的先后画她说的话，「施工时定的」第 84 条），由这一头记
    /// 结局（施工 O-25 中，`sending`）。放下 `Route` 时不掐：任务只等回应，连接断了、到了时限自己就完。
    sending: FuturesOrdered<JoinHandle<Answered>>,
    /// 私聊会话编号 → 桥这一轮自己入队了的（施工 O-25 上，`outbound`：私聊的出站链照它去重；O-25 中改成入队时记）。
    spoken: HashMap<String, Spoken>,
    /// 群里的命令回执：NapCat 回了编号以后等几秒撤回的任务（施工 O-25 上，`receipt`）。不放进 [`Route::sending`]：照先后交的
    /// 那一串会被它卡住几秒。
    recalls: JoinSet<()>,
    /// 群里的命令回执发出去几秒后撤回（`bridge.json` 的 `receipt_recall_seconds`）。
    recall: Duration,
    /// 在判的和问判官的任务（施工 O-23 下）。
    judges: Judges,
    /// 推来的配置变化交给 `serve`（施工 O-20）。
    configured: mpsc::UnboundedSender<Map<String, Value>>,
}

impl Route {
    /// 拿着连接 `core`，回话照 `bots` 找连接，场所规则照 `rules`，群成员的名字记进 `members`，发进群里的提示照 `texts`
    /// 说，问判官照全局的名额 `slots` 排队（施工 O-23 下）、判官带的人格原文照 `personas` 记（施工 O-23 补），群里的命令回执
    /// `recall` 以后撤回（施工 O-25 上），出站排着的过了 `expire` 作废（施工 O-25 中），推来的配置变化交给 `configured`。自己人
    /// 照握手交来的配置（`core.config`）。
    pub(crate) fn new(
        core: Core,
        bots: Arc<Bots>,
        rules: Venues,
        members: Members,
        (texts, slots, personas, recall, expire): (Texts, Slots, Personas, Duration, Duration),
        configured: mpsc::UnboundedSender<Map<String, Value>>,
    ) -> Route {
        let trusted = trusted(&core.config[trusted_key()]);
        let judges = Judges::new(core.caller(), rules.judge_texts(), slots, personas);
        Route {
            core,
            bots,
            rules,
            members,
            venues: HashMap::new(),
            peers: HashMap::new(),
            refused: HashSet::new(),
            groups: HashMap::new(),
            trusted,
            texts,
            waiting: Queue::new(expire),
            sending: FuturesOrdered::new(),
            spoken: HashMap::new(),
            recalls: JoinSet::new(),
            recall,
            judges,
            configured,
        }
    }

    /// 一直办，直到办不下去：核心关了管道、写不出去（核心请它退出、核心不在了，第 11 条），交回空的，桥好好停下；发回话
    /// 的任务、问判官的任务（施工 O-23 下）、撤回执的任务（施工 O-25 上）崩了，交回 [`Failure::Crashed`]（「施工时定的」
    /// 第 14 条），桥照它退出。`inbound` 关了就只看核心：桥在停、读 NapCat 的那一头都放下了。有东西排着的，另睡到最早的过期、
    /// 禁言到期的时刻，醒了再看一遍（施工 O-25 中，「出站队列」第 8 条）。
    pub(crate) async fn run(mut self, mut inbound: mpsc::Receiver<Event>) -> Option<Failure> {
        let mut open = true;
        loop {
            let wake = self.wake();
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
                Some(joined) = self.sending.next(), if !self.sending.is_empty() => match sent(joined) {
                    Ok(answered) => self.answered(answered).await.map_err(|Gone| None),
                    Err(failure) => Err(Some(failure)),
                },
                Some(joined) = self.recalls.join_next(), if !self.recalls.is_empty() => {
                    sent(joined).map_err(Some)
                }
                () = tokio::time::sleep(wake.unwrap_or_default()), if wake.is_some() => {
                    self.pump().await.map_err(|Gone| None)
                }
                Some(joined) = self.judges.running.join_next(), if !self.judges.running.is_empty() => match joined {
                    Ok(asked) => self.judged(asked).await.map_err(|Gone| None),
                    Err(error) => {
                        tracing::error!(target: TARGET, error = %error, "judge task crashed");
                        Err(Some(Failure::Crashed(error.to_string())))
                    }
                },
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
            Event::Muted {
                bot,
                group,
                seconds,
            } => self.muted(bot, group, Some(seconds)).await,
            Event::Unmuted { bot, group } => self.muted(bot, group, None).await,
            // 号连上了：排着的照先后发（施工 O-25 中，「出站队列」第 3 条）。
            Event::Connected { .. } => self.pump().await,
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
        self.submit(message).await.map(|_| ())
    }

    /// 交一条消息：`/` 开头的先当斜杠命令交（「斜杠命令」），办完了的不再发；别的照 `session.send` 发（第 8 条，「群消息」
    /// 第 6 条），交回会话编号和回应（接受的、拒绝的都原样：群的照它判，施工 O-23）。命令办完了的、不接的、找不到会话的是
    /// 空的。
    async fn submit(&mut self, message: Message) -> Result<Option<(String, Value)>, Gone> {
        if command::looks_like(&message.text) && self.command(&message).await? {
            return Ok(None);
        }
        let Some((session, reply)) = self.deliver(&message, session::SEND).await? else {
            return Ok(None);
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
        Ok(Some((session, reply)))
    }

    /// 这一刻套到场所 `venue` 上的场所规则（「场所规则和出厂数据」第 3、5 条）。
    fn applied(&mut self, venue: &Venue) -> Applied {
        self.rules.current(Instant::now()).at(venue)
    }

    /// 核心推来的一条：配置变了的（`extension.config`）交给 `serve`（施工 O-20），自己人在里面的换上（施工 O-23）；群会话的
    /// 事件收进投影、她新说的话发回群里（施工 O-23）；私聊里她的回话发回去（第 10 条）；掉了队、会话停了的（`resync`）再订阅
    /// 一次，群的照收到的最后一条接着补。
    async fn pushed(&mut self, pushed: Value) -> Result<(), Gone> {
        let params = &pushed["params"];
        if pushed["method"] == "extension.config" {
            let Some(keys) = params["keys"].as_object() else {
                return Ok(());
            };
            if let Some(value) = keys.get(&trusted_key()) {
                self.trusted = trusted(value);
                tracing::info!(target: TARGET, count = self.trusted.len(), "trusted changed");
            }
            if self.configured.send(keys.clone()).is_err() {
                // `serve` 不收了：桥在停，没有别处可交。
            }
            return Ok(());
        }
        let Some(session) = params["session"].as_str() else {
            return Ok(());
        };
        let group = self.groups.get(session).map(Projection::last);
        match (pushed["method"].as_str(), group) {
            (Some("event"), Some(_)) => {
                let session = session.to_string();
                self.heard(&session, &pushed).await?;
            }
            (Some("resync"), Some(last)) if params["stream"] == "events" => {
                let session = session.to_string();
                self.follow(&session, last).await?;
            }
            (Some("event"), None) if params["event"]["kind"] == "message.assistant" => {
                let session = session.to_string();
                self.say_privately(&session, &params["event"]).await?;
            }
            (Some("resync"), None) if params["stream"] == "events" => {
                let session = session.to_string();
                self.subscribe(&session).await?;
            }
            _ => {}
        }
        Ok(())
    }
}

/// 一个发回话、撤回执的任务结束了：交回它交出来的（等来的结局，施工 O-25 中）。崩了（是 bug：任务里只等回应、等几秒撤）照实
/// 交上去，桥照「施工时定的」第 14 条停下，不装作还在跑；记一行运行日志。任务不会被掐掉：没有人掐它们。
fn sent<T>(joined: Result<T, JoinError>) -> Result<T, Failure> {
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

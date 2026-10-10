//! 消息进来、回话出去（`onebot.md` 第一条「怎么走」第 6 到 10 条，「群消息」「撤回」「好友请求」）：一个任务拿着跟核心的连接，
//! 一边收 NapCat 那头读出来的消息、撤回和请求，一边收核心推来的事件。一件件照先后办（套场所规则、找会话、发进去），办的时候来的推送由
//! [`Core`] 留着、办完再看。
//!
//! - 私聊（这里）：场所、平台上的人经群聊内核拼（`onebot::venue`、`onebot::person`），拼不出来的（照说不会）记一行、这条
//!   不送；带上场所的格（施工 O-22，`fields`），`show_ids` 照这个私聊套出来的场所规则（`applied`）。白名单成员的私聊照场所
//!   规则带人格、预设、工作区（施工 O-27，同群）。
//! - 群消息（`group`，施工 O-22）：一律旁听；正文里的 @ 写成名字（`names`）；场所规则管人格、预设、工作区、谁是管理的人、
//!   睡没睡、看不看得到号。撤回（`recall`，施工 O-22）记 `venue.recalled`。
//! - 群里叫她（施工 O-23，「群里怎么叫她」）：记下以后判（`called`：纯逻辑的判断在 `decide`，线路规程在 `discipline`，
//!   判断的样子在 `body`），该回的开一轮；群会话推来的事件收进投影（`projection`），她新说的话过出站链（`outbound`，施工
//!   O-25 上）发回群里（`speak`）。要问判官的（O-23 下）交给 `judges`，任务（`ask`）经并着发的调用口
//!   问，跟核心的那一头接着办别的；判官回来了照号收回来、算分、记判断（`judged`）。判官带这个群会话所用的人格的说明（O-23
//!   补，`persona`）。
//! - 起来就订阅（施工 O-32，`revive`）：每次连上核心、握手以后先经 `venue.sessions` 列出名下的场所会话和终端管理员的私聊，群的
//!   从头订阅，白名单成员的、终端管理员的私聊也从头订阅一次；补来的（`backlog`）认出发到哪，期限以内、没入队的她的话补发。
//! - 找会话（`session`）：「场所 → 会话编号」只记在内存里，每个场所桥起来以后第一次要用时问一次 `venue.session`（起来时订阅了的
//!   群不再问）；私聊的问到了订阅（不写 `after`），群的从头订阅（施工 O-23）。会话不在了（`session_not_found`、`session_stopped`）
//!   忘掉，再问一次、再交一次，只重来一次。私聊不是终端管理员、也不在白名单里的（`no_system_account`，或者会话的属主是桥
//!   自己，「施工时定的」第 49 条，施工 O-27）、群的规则写错的不接，同一个场所只记一行运行日志。
//! - 好友请求（施工 O-27，`request`）：白名单成员的另起任务同意，别的放着；群邀请只记一行。
//! - `/` 开头的先当斜杠命令交 `command.run`（`command`，O-19，第 7、8 条之间的「斜杠命令」），核心说不是命令的才照普通的话发；
//!   回执交 `receipt`，群里的过几秒撤回（施工 O-25 上）。
//! - 她在私聊里的回话：`message.assistant` 的文字块接起来，过出站链、拆段（`speak`，施工 O-25 上）。
//! - 出站队列（施工 O-25 中，「出站队列」）：她的话、提示、回执都先入队（记 `ext.onebot.venues.queued`）再照先后交给收进这个会话
//!   的那个机器人号现在的连接，等回应的那一步交给别的任务，结局照放进写队列的先后交回来记（`sending`；排着、过期的纯逻辑在
//!   `queue`）。她被禁言、解禁的通知记下（`muted`），禁言时这个群的出站排着；号连上了、定时醒了再看一遍排着的。她的话没发出去的
//!   经核心的 `session.note` 退信（施工 O-25 下，「退信」，在 `sending`）。
//! - 贴表情（施工 O-25 下，「贴表情」）：判下来要回、主触发是冲她来或续聊的，在她要回的那一条上贴，那一轮发出去第一段、结束了、
//!   到时候了摘（`reaction`）。
//! - 不说话（施工 O-26，「提供者和不说话」）：她的回复里有 `skip_reply` 的调用块，这一轮的字都不发（`quiet` 认，`speak` 不发）。
//! - 平台工具（一）（施工 O-31，「平台工具（一）」）：读的一头交来的撤回、禁言、戳一戳的 `tool.call`，做不做、对谁做照 `platform`
//!   定，`acting` 收投影、调 NapCat、答核心。
//! - 核心推来的 `extension.config`（施工 O-20）：`keys` 原样交给 `serve`，它换上手里的配置、端口变了另起一个任务换（`running::rebind`）
//!   （「施工时定的」第 45 条）：这里够不着监听。白名单成员 `onebot.whitelist` 这里记一份（施工 O-23）；换了的，私聊找过的
//!   会话都忘掉，下一条照新的白名单再找（施工 O-27）。

mod acting;
mod applied;
mod ask;
mod backlog;
mod bindings;
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
mod platform;
mod projection;
mod queue;
mod quiet;
mod reaction;
mod recall;
mod receipt;
mod request;
mod revive;
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
use miyu_kernel::id::{ExternalId, VenueId};
use serde_json::{Map, Value, json};
use tokio::sync::mpsc;
use tokio::task::{JoinError, JoinHandle, JoinSet};

use crate::TARGET;
use crate::core::{Core, Gone, reason};
use crate::listen::bots::Bots;
use crate::onebot::{Event, Members, Posted, To, command_id, person, venue};
use crate::rules::{Applied, Tools, Venues};
use crate::serve::Failure;
use crate::settings::{whitelist, whitelist_key};
use crate::texts::Texts;
pub(crate) use ask::Slots;
use backlog::Backlog;
use bindings::Bindings;
use fields::{Flags, fields};
use judges::Judges;
use outbound::Spoken;
pub(crate) use persona::Personas;
use projection::Projection;
use queue::Queue;
use quiet::Quiet;
pub(crate) use reaction::Reactions;
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
    /// 记过一行「不接」的场所（第 7 条、「群消息」第 3 条）：私聊不是终端管理员、也不在白名单里的，群的规则写错的。
    refused: HashSet<String>,
    /// 群会话编号 → 这个群的投影（施工 O-23，「群里怎么叫她」第 1、2 条）：订阅了的群才有。
    groups: HashMap<String, Projection>,
    /// 白名单成员的平台身份（`onebot.whitelist`，施工 O-23）：握手交来的，推来新的就换。
    whitelist: Vec<String>,
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
    /// 另起的平台动作：群里的命令回执 NapCat 回了编号以后等几秒撤回（施工 O-25 上，`receipt`），贴了表情等着摘（施工 O-25 下，
    /// `reaction`）。不放进 [`Route::sending`]：照先后交的那一串会被它们卡住。
    chores: JoinSet<()>,
    /// 群里的命令回执发出去几秒后撤回（`bridge.json` 的 `receipt_recall_seconds`）。
    recall: Duration,
    /// 在判的和问判官的任务（施工 O-23 下）。
    judges: Judges,
    /// 贴着的表情（施工 O-25 下，`reaction`）。
    reactions: Reactions,
    /// 这一轮不说话了的（施工 O-26，`quiet`）：群里、私聊的都在这一张表里。
    quiet: Quiet,
    /// 桥的工具（施工 O-31）：平台工具答的话。
    tools: Arc<Tools>,
    /// 问到的「是不是终端管理员」（施工 O-31，`bindings`）。
    bindings: Bindings,
    /// 私聊会话编号 → 推来的最近一条人说的话引用的平台编号（没引用的是空的，施工 O-31，`acting`：私聊里叫她做的那条）。
    quotes: HashMap<String, Option<String>>,
    /// 会话编号 → 订阅补来的那一段（施工 O-32，`backlog`）：只在订阅到补完的这一会儿有，补完了补发、拿掉。
    backlogs: HashMap<String, Backlog>,
    /// 推来的配置变化交给 `serve`（施工 O-20）。
    configured: mpsc::UnboundedSender<Map<String, Value>>,
}

impl Route {
    /// 拿着连接 `core`，回话照 `bots` 找连接，场所规则照 `rules`，群成员的名字记进 `members`，发进群里的提示照 `texts`
    /// 说，问判官照全局的名额 `slots` 排队（施工 O-23 下）、判官带的人格原文照 `personas` 记（施工 O-23 补），群里的命令回执
    /// `recall` 以后撤回（施工 O-25 上），出站排着的过了 `expire` 作废（施工 O-25 中），表情照 `reactions` 贴、摘（施工 O-25 下），
    /// 推来的配置变化交给 `configured`。白名单成员照握手交来的配置（`core.config`）。问到的「是不是终端管理员」记 `binding`（施工
    /// O-31）。
    pub(crate) fn new(
        core: Core,
        bots: Arc<Bots>,
        rules: Venues,
        members: Members,
        (texts, slots, personas, recall, expire, reactions, binding): (
            Texts,
            Slots,
            Personas,
            Duration,
            Duration,
            Reactions,
            Duration,
        ),
        configured: mpsc::UnboundedSender<Map<String, Value>>,
    ) -> Route {
        let whitelist = whitelist(&core.config[whitelist_key()]);
        let judges = Judges::new(core.caller(), rules.judge_texts(), slots, personas);
        let tools = rules.tools();
        Route {
            core,
            bots,
            rules,
            members,
            venues: HashMap::new(),
            peers: HashMap::new(),
            refused: HashSet::new(),
            groups: HashMap::new(),
            whitelist,
            texts,
            waiting: Queue::new(expire),
            sending: FuturesOrdered::new(),
            spoken: HashMap::new(),
            chores: JoinSet::new(),
            recall,
            judges,
            reactions,
            quiet: Quiet::default(),
            tools,
            bindings: Bindings::new(binding),
            quotes: HashMap::new(),
            backlogs: HashMap::new(),
            configured,
        }
    }

    /// 先订阅名下的场所会话（施工 O-32，`revive`），再一直办，直到办不下去：核心关了管道、写不出去（核心请它退出、核心不在了，
    /// 第 11 条），交回空的，桥好好停下；发回话
    /// 的任务、问判官的任务（施工 O-23 下）、撤回执、贴表情的任务（施工 O-25 上、下）崩了，交回 [`Failure::Crashed`]（「施工时定的」
    /// 第 14 条），桥照它退出。`inbound` 关了就只看核心：桥在停、读 NapCat 的那一头都放下了。有东西排着的，另睡到最早的过期、
    /// 禁言到期的时刻，醒了再看一遍（施工 O-25 中，「出站队列」第 8 条）。
    pub(crate) async fn run(mut self, mut inbound: mpsc::Receiver<Event>) -> Option<Failure> {
        // 起来就订阅（施工 O-32，「群里怎么叫她」第 1 条）：NapCat 那头来的先排着，订阅完了再办。
        if self.revive().await.is_err() {
            return None;
        }
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
                Some(joined) = self.chores.join_next(), if !self.chores.is_empty() => {
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
            Event::Befriend { bot, user, flag } => {
                self.befriend(bot, user, flag);
                Ok(())
            }
            Event::Invited { bot, group, user } => {
                request::invited(bot, group, user);
                Ok(())
            }
        }
    }

    /// 平台上的 `who` 在不在白名单里（`onebot.whitelist`，施工 O-23、O-27）。
    fn whitelisted(&self, who: &ExternalId) -> bool {
        self.whitelist.iter().any(|one| one == who.as_str())
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
        let applied = self.applied(&venue);
        let flags = Flags {
            show_ids: applied::show_ids(&applied),
            ..Flags::default()
        };
        // 白名单成员的私聊会话归系统账号、照场所规则造（施工 O-27，同群）。
        let listed = self.whitelisted(&external).then_some(&applied);
        let message = Message {
            id: command_id(posted.bot, posted.message_id, posted.time),
            number: posted.message_id,
            text: posted.text.clone(),
            acting: json!({"external": external}),
            fields: fields(&posted, &[], flags),
            place: Place::private(&venue, &external, (posted.bot, posted.user), listed),
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

    /// 核心推来的一条：配置变了的（`extension.config`）交给 `serve`（施工 O-20），白名单成员在里面的换上（施工 O-23），私聊找过的
    /// 会话都忘掉（施工 O-27：删了的人下一条照新的白名单再找，就不接了；终端管理员的找回来还是那一个，再订阅一次不重）；群会话的
    /// 事件收进投影、她新说的话发回群里（施工 O-23）；私聊会话的事件交给 `say_privately`：她的回话发回去（第 10 条），这一轮
    /// 不说话了的不发（施工 O-26，要看 `turn.ended`）；掉了队、会话停了的（`resync`）再订阅一次，群的照收到的最后一条接着补。
    /// 读的一头交来的平台工具的调用（`tool.call`，施工 O-31）交给 `acting`。
    async fn pushed(&mut self, pushed: Value) -> Result<(), Gone> {
        let params = &pushed["params"];
        if pushed["method"] == "extension.config" {
            let Some(keys) = params["keys"].as_object() else {
                return Ok(());
            };
            if let Some(value) = keys.get(&whitelist_key()) {
                self.whitelist = whitelist(value);
                let groups = &self.groups;
                self.venues
                    .retain(|_, session| groups.contains_key(session));
                tracing::info!(target: TARGET, count = self.whitelist.len(), "whitelist changed");
            }
            if self.configured.send(keys.clone()).is_err() {
                // `serve` 不收了：桥在停，没有别处可交。
            }
            return Ok(());
        }
        if pushed["method"] == "tool.call" {
            return self.tool_called(&pushed).await;
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
            (Some("event"), None) => {
                let session = session.to_string();
                self.say_privately(&session, &params["event"]).await?;
            }
            (Some("resync"), None) if params["stream"] == "events" => {
                let session = session.to_string();
                self.subscribe(&session, None).await?;
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

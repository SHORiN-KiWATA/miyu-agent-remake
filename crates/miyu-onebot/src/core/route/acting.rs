//! 平台工具（一）的那一头（施工 O-31，`onebot.md` 第一条「平台工具（一）」第 2、5、6 条）：读的一头把撤回、禁言、戳一戳的
//! `tool.call` 照推送交到这里。先把这个会话留着的推送收进投影（私聊的照「怎么走」第 9 条那条路收，记下最近一条的引用），认出
//! 叫她做的那条、谁叫的；群里叫的人和那条牵涉的人是不是终端管理员，记着的照记的，没记着的问核心的 `venue.binding`（`bindings`，
//! 「施工时定的」第 183 条）；交 `platform.rs` 定做不做、对谁做，不做的当场答。做的另起一个任务（`chores`，同贴表情）：禁言要问
//! 身份的先问 `get_group_member_info`，再经收进这个会话的那个机器人号那时的连接调动作，答核心。跟核心的那一头不等 NapCat
//! （「施工时定的」第 178 条）。

use std::sync::Arc;
use std::time::Instant;

use serde_json::json;

use miyu_kernel::id::ExternalId;
use serde_json::Value;

use super::fields::{NAME, clean};
use super::platform::{Caller, Order, Origin, Plan, Quote, Scene, Why, duration, plan};
use super::projection::Projection;
use super::queue::DETAIL;
use super::{Peer, Route};
use crate::TARGET;
use crate::core::{Answerer, Gone, reason};
use crate::listen::bots::Bots;
use crate::onebot::{
    CallError, MEMBER_INFO, Rank, To, delete_msg, group_ban, member_info, person, poke, rank_of,
};
use crate::rules::Tools;

/// 要调的平台动作（号已经解成整数）。
#[derive(Debug, Clone)]
enum Act {
    /// 撤这一条（平台编号原样）。
    Recall(String),
    /// 在这个群里禁言这个号几秒；`check` 的先问身份，群主、群管理员不动。
    Mute {
        /// 群号。
        group: i64,
        /// 禁谁。
        user: i64,
        /// 几秒，0 是解禁。
        seconds: u64,
        /// 先问身份。
        check: bool,
    },
    /// 戳这个号。
    Poke(i64),
}

/// 交给任务办的一次调用。
struct Job {
    /// `tool.call` 的编号：照原样答。
    id: Value,
    /// 工具名：记运行日志。
    tool: String,
    /// 收进这个会话的机器人号、发到哪、场所。
    peer: Peer,
    /// 调什么。
    act: Act,
    /// 结果里写的人：名字，没有的写号（「施工时定的」第 185 条）。
    who: String,
    /// 机器人号的连接。
    bots: Arc<Bots>,
    /// 答的话。
    tools: Arc<Tools>,
    /// 往核心写回应的一头。
    answerer: Answerer,
}

impl Route {
    /// 核心转来的一次平台工具的调用 `pushed`（`tool.call`，「平台工具（一）」第 2 到 5 条）。桥不认识这个会话的答
    /// `unreachable`；私聊里调到禁言的（`venues` 不给，照说不会）照不认识的工具答。
    ///
    /// # Errors
    ///
    /// 收这个会话留着的推送时写不出去、核心断开（同 [`Route::catch_up`]）。
    pub(super) async fn tool_called(&mut self, pushed: &Value) -> Result<(), Gone> {
        let params = &pushed["params"];
        let (id, tool) = (&pushed["id"], params["tool"].as_str().unwrap_or_default());
        let session = params["session"].as_str().unwrap_or_default().to_string();
        let answerer = self.core.answerer();
        self.gather(&session).await?;
        let Some(peer) = self.peers.get(&session).cloned() else {
            tracing::warn!(target: TARGET, session, tool, why = "unreachable", "platform tool failed");
            answerer.answer(id, self.tools.answer("unreachable", &[], true));
            return Ok(());
        };
        let caller = Caller::read(params);
        if matches!(peer.to, To::Group(_)) {
            self.ask_bindings(&session, &caller).await?;
        }
        let decided = Order::read(tool, &params["args"])
            .and_then(|order| self.decide(&session, &peer, order, &caller));
        let Some(decided) = decided else {
            answerer.answer(id, self.tools.call(tool));
            return Ok(());
        };
        let act = match self.act(&peer, decided) {
            Ok(act) => act,
            Err(why) => {
                tracing::info!(target: TARGET, venue = %peer.venue, tool, why = why.name(), "platform tool refused");
                let who = match &why {
                    Why::Protected(user) => self.who(&peer, user),
                    _ => String::new(),
                };
                answerer.answer(id, self.tools.answer(why.name(), &[("who", &who)], true));
                return Ok(());
            }
        };
        let who = match &act {
            Act::Recall(_) => String::new(),
            Act::Mute { user, .. } | Act::Poke(user) => self.who_number(&peer, *user),
        };
        let job = Job {
            id: id.clone(),
            tool: tool.to_string(),
            peer,
            act,
            who,
            bots: Arc::clone(&self.bots),
            tools: Arc::clone(&self.tools),
            answerer,
        };
        self.chores.spawn(run(job));
        Ok(())
    }

    /// 会话 `session`（发到 `peer`）里 `caller` 叫她做的 `order` 做不做、对谁做（「平台工具（一）」第 3、4 条）：群里叫她做的
    /// 那条照投影，私聊照最近一条的引用。私聊里的禁言是空的。
    fn decide(&self, session: &str, peer: &Peer, order: Order, caller: &Caller) -> Option<Plan> {
        let me = person(peer.bot).ok()?;
        match peer.to {
            To::Group(_) => {
                let projection = self.groups.get(session);
                let now = Instant::now();
                // 终端管理员：对应表里有的（问过核心的），或者在这个群说过的话带 `account` 的（问不到时兜底）。
                let admin = |who: &ExternalId| {
                    self.bindings.known(who.as_str(), now) == Some(true)
                        || projection.is_some_and(|one| one.is_admin(who))
                };
                let caller = &Caller {
                    manager: caller.manager || caller.id.as_ref().is_some_and(admin),
                    id: caller.id.clone(),
                };
                let scene = Scene {
                    peer: None,
                    me: &me,
                    whitelist: &self.whitelist,
                    admin: &admin,
                };
                let origin = projection.and_then(Projection::origin);
                Some(plan(order, caller, origin.as_ref(), &scene))
            }
            To::Private(_) if matches!(order, Order::Mute(_)) => None,
            To::Private(user) => {
                let other = person(user).ok()?;
                let quote = self.quotes.get(session).cloned().flatten();
                let origin = Origin {
                    sender: other.clone(),
                    quote: quote.map(|msg| Quote {
                        msg,
                        mine: false,
                        sender: None,
                    }),
                    mentions: Vec::new(),
                };
                let nobody = |_: &ExternalId| false;
                let scene = Scene {
                    peer: Some(&other),
                    me: &me,
                    whitelist: &self.whitelist,
                    admin: &nobody,
                };
                Some(plan(order, caller, Some(&origin), &scene))
            }
        }
    }

    /// 群会话 `session` 里叫的人 `caller` 和叫她做的那条牵涉的人（发它的人、引用的那一条的发的人、@ 的人），没记着是不是终端
    /// 管理员的问核心的 `venue.binding`，问到的记下（「平台工具（一）」第 3 条）。核心拒了的（照说不会：桥是系统账号）记一行
    /// `WARN binding not asked`、不记，这一次照投影兜底。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    async fn ask_bindings(&mut self, session: &str, caller: &Caller) -> Result<(), Gone> {
        let origin = self.groups.get(session).and_then(Projection::origin);
        let mut people: Vec<ExternalId> = caller.id.iter().cloned().collect();
        if let Some(origin) = origin {
            people.push(origin.sender);
            people.extend(origin.quote.and_then(|quote| quote.sender));
            people.extend(origin.mentions);
        }
        for who in people {
            if self.bindings.known(who.as_str(), Instant::now()).is_some() {
                continue;
            }
            let reply = self
                .core
                .call("venue.binding", json!({"id": who.as_str()}))
                .await?;
            match reason(&reply) {
                None => {
                    let admin = reply["result"]["account"].is_string();
                    self.bindings.note(who.as_str(), admin, Instant::now());
                }
                Some(reason) => {
                    tracing::warn!(target: TARGET, id = who.as_str(), reason, "binding not asked");
                }
            }
        }
        Ok(())
    }

    /// 把会话 `session` 留着的推送收进来：群的进投影（同「群里怎么叫她」第 3 条），私聊的照「怎么走」第 9 条那条路（记下最近一条
    /// 的引用）。
    pub(super) async fn gather(&mut self, session: &str) -> Result<(), Gone> {
        if self.groups.contains_key(session) {
            return self.catch_up(session).await;
        }
        for pushed in self.core.take_events(session) {
            self.say_privately(session, &pushed["params"]["event"])
                .await?;
        }
        Ok(())
    }

    /// 定下来的 `plan` 换成要调的动作：号解成整数；禁言秒数大于 0 的，群成员的缓存里有身份的当场挡群主、群管理员，没有的交给
    /// 任务问。不做的交回为什么。
    fn act(&self, peer: &Peer, plan: Plan) -> Result<Act, Why> {
        let number = |user: &ExternalId| {
            miyu_chat::parse_person(user)
                .and_then(|(_, number)| number.parse::<i64>().ok())
                .ok_or(Why::NoTarget)
        };
        match plan {
            Plan::Refuse(why) => Err(why),
            Plan::Recall(msg) => Ok(Act::Recall(msg)),
            Plan::Poke(user) => Ok(Act::Poke(number(&user)?)),
            Plan::Mute { user, seconds } => {
                let id = number(&user)?;
                // 私聊没有禁言（`venues` 只给群，调的一方挡在前面）。
                let To::Group(group) = peer.to else {
                    return Err(Why::NoTarget);
                };
                let rank = self.members.rank(group, id, Instant::now());
                if seconds > 0 && matches!(rank, Some(Rank::Owner | Rank::Admin)) {
                    return Err(Why::Protected(user));
                }
                Ok(Act::Mute {
                    group,
                    user: id,
                    seconds,
                    check: seconds > 0 && rank.is_none(),
                })
            }
        }
    }

    /// 结果里写的人 `user`：同 [`Route::who_number`]，号解不出的写原样。
    fn who(&self, peer: &Peer, user: &ExternalId) -> String {
        miyu_chat::parse_person(user)
            .and_then(|(_, number)| number.parse::<i64>().ok())
            .map_or_else(|| user.as_str().to_string(), |id| self.who_number(peer, id))
    }

    /// 结果里写的号是 `user` 的人：群里照群成员缓存里的名字（洗过），没有的、私聊里的写号（「施工时定的」第 185 条）。
    fn who_number(&self, peer: &Peer, user: i64) -> String {
        let name = match peer.to {
            To::Group(group) => self
                .members
                .name(group, user, Instant::now())
                .and_then(|name| clean(name, NAME)),
            To::Private(_) => None,
        };
        name.unwrap_or_else(|| user.to_string())
    }
}

/// 没做成的。
#[derive(Debug, PartialEq, Eq)]
enum Undone {
    /// 问了身份：是群主、群管理员。
    Protected,
    /// NapCat 没成：答的那一句的名字（`failed`、`unreachable`、`unanswered`），`failed` 的带它说的原因。
    Failed(&'static str, Option<String>),
}

/// 办一次调用：要问身份的先问，再调动作，答核心、记运行日志（「平台工具（一）」第 5、6 条）。
async fn run(job: Job) {
    let venue = &job.peer.venue;
    let who = job.who.as_str();
    let seconds;
    let outcome = act(&job).await;
    let (name, fields, error): (&str, Vec<(&str, &str)>, bool) = match &outcome {
        Ok(()) => {
            tracing::info!(target: TARGET, venue = %venue, tool = job.tool, "platform tool done");
            match &job.act {
                Act::Recall(_) => ("recalled", Vec::new(), false),
                Act::Mute { seconds: 0, .. } => ("unmuted", vec![("who", who)], false),
                Act::Mute { seconds: muted, .. } => {
                    seconds = duration(*muted);
                    ("muted", vec![("who", who), ("duration", &seconds)], false)
                }
                Act::Poke(_) => ("poked", vec![("who", who)], false),
            }
        }
        Err(Undone::Protected) => {
            tracing::info!(target: TARGET, venue = %venue, tool = job.tool, why = "protected", "platform tool refused");
            ("protected", vec![("who", who)], true)
        }
        Err(Undone::Failed(why, detail)) => {
            tracing::warn!(target: TARGET, venue = %venue, tool = job.tool, why, "platform tool failed");
            let fields = detail.as_deref().map(|detail| ("detail", detail));
            (why, fields.into_iter().collect(), true)
        }
    };
    job.answerer
        .answer(&job.id, job.tools.answer(name, &fields, error));
}

/// 调 `job` 的动作，经那个机器人号这时的连接；禁言要问身份的先问（「施工时定的」第 184 条：回了成功、没有 `role` 的当普通
/// 成员）。
async fn act(job: &Job) -> Result<(), Undone> {
    let Some(link) = job.bots.get(job.peer.bot) else {
        return Err(Undone::Failed("unreachable", None));
    };
    let (action, params) = match &job.act {
        Act::Recall(msg) => delete_msg(msg.as_str()),
        Act::Mute {
            group,
            user,
            seconds,
            check,
        } => {
            if *check {
                let asked = link
                    .calls
                    .call(&link.out, MEMBER_INFO, member_info(*group, *user));
                let reply = asked.await.map_err(undone)?;
                if matches!(rank_of(&reply["data"]), Some(Rank::Owner | Rank::Admin)) {
                    return Err(Undone::Protected);
                }
            }
            group_ban(*group, *user, *seconds)
        }
        Act::Poke(user) => poke(job.peer.to, *user),
    };
    link.calls
        .call(&link.out, action, params)
        .await
        .map(|_| ())
        .map_err(undone)
}

/// NapCat 没成算成哪一句：回了失败的是 `failed`，原因照它的 `message`（空的换 `wording`，再空的写 `retcode`）去掉首尾空白、截到
/// [`DETAIL`] 个字符；等不到是 `unanswered`；没连着、断了是 `unreachable`。
fn undone(error: CallError) -> Undone {
    match error {
        CallError::Failed(reply) => {
            let said = ["message", "wording"]
                .iter()
                .filter_map(|key| reply[*key].as_str())
                .map(str::trim)
                .find(|said| !said.is_empty())
                .map_or_else(|| reply["retcode"].to_string(), str::to_string);
            Undone::Failed("failed", Some(said.chars().take(DETAIL).collect()))
        }
        CallError::Timeout => Undone::Failed("unanswered", None),
        CallError::Closed => Undone::Failed("unreachable", None),
    }
}

#[cfg(test)]
mod tests;

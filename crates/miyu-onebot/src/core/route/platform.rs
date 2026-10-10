//! 平台工具（一）做不做、对谁做（施工 O-31，`onebot.md` 第一条「平台工具（一）」第 3、4 条；18 第十一节）：纯逻辑。
//!
//! 目标从叫她做的那条取（[`Origin`]：群里是投影里主线这一轮她回的那一条，私聊是最近一条人说的话），模型不给编号、不给人：撤回
//! 的目标以回复的那条为准。叫她做的那条和 `tool.call` 的 `by` 对不上的当没有：A 的身份配不上 B 的引用（「施工时定的」第 179 条）。
//! 谁能叫：撤她自己的、戳一戳谁都行；撤别人的、禁言只给管理的人（`owner`，或者 `by.role` 是 `manager`）。禁言不动终端管理员、
//! 白名单成员（这里挡），群主、群管理员要问身份（[`Plan::Mute`] 交出去以后由 `acting.rs` 挡）；解禁谁都解（第 181 条）。

use miyu_kernel::id::ExternalId;
use serde_json::Value;

use crate::rules::{MUTE, POKE, RECALL};

/// 禁言最多多少秒：QQ 的上限，30 天（施工单「要定的」第 2 条）。是平台定的，不是桥的参数。
const MAX_SECONDS: u64 = 2_592_000;

/// 叫她做的一件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Order {
    /// 撤回。
    Recall,
    /// 禁言几秒（0 是解禁）；没写、不是整数、出了范围的是空的。
    Mute(Option<u64>),
    /// 戳一戳。
    Poke,
}

impl Order {
    /// 工具 `tool`、参数 `args`：不是这三件的是空的。
    pub(super) fn read(tool: &str, args: &Value) -> Option<Order> {
        match tool {
            RECALL => Some(Order::Recall),
            MUTE => {
                let seconds = args["seconds"]
                    .as_u64()
                    .filter(|seconds| *seconds <= MAX_SECONDS);
                Some(Order::Mute(seconds))
            }
            POKE => Some(Order::Poke),
            _ => None,
        }
    }
}

/// 谁叫的（`tool.call` 的 `by`、`owner`，`providers.md`「是谁要的」）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Caller {
    /// 平台上的身份：外部身份的 `by.id`；私聊里核心照对应表认出的终端管理员记成本人，照它经的平台身份 `by.via`。没有触发的回合、
    /// 本机的人是空的。
    pub(super) id: Option<ExternalId>,
    /// 管理的人：`owner` 是真（终端管理员，核心照对应表认的），或者 `by.role` 是 `manager`（场所规则的 `managers`）。
    pub(super) manager: bool,
}

impl Caller {
    /// 照 `tool.call` 的参数 `params` 认。
    pub(super) fn read(params: &Value) -> Caller {
        let by = &params["by"];
        let id = match by["kind"].as_str() {
            Some("external") => by["id"].as_str(),
            Some("person") => by["via"].as_str(),
            _ => None,
        };
        let id = id.and_then(|id| ExternalId::parse(id).ok());
        Caller {
            id,
            manager: params["owner"] == true || by["role"] == "manager",
        }
    }
}

/// 叫她做的那条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Origin {
    /// 发它的人。
    pub(super) sender: ExternalId,
    /// 它引用的那一条；没引用的是空的。
    pub(super) quote: Option<Quote>,
    /// 它 @ 了谁（不含她：群消息记的时候就去掉了，「群消息」第 2 条），照先后。
    pub(super) mentions: Vec<ExternalId>,
}

/// 被引用的那一条。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Quote {
    /// 平台编号，照引用里的原样：撤它时原样还回去。
    pub(super) msg: String,
    /// 是她发的（投影里她发过的编号）。
    pub(super) mine: bool,
    /// 发它的人：投影里认得出的；她的、桥不认识的是空的。
    pub(super) sender: Option<ExternalId>,
}

/// 这个场所的情形。
pub(super) struct Scene<'a> {
    /// 私聊的对方；群的是空的。
    pub(super) peer: Option<&'a ExternalId>,
    /// 她自己（收进这个会话的机器人号）。
    pub(super) me: &'a ExternalId,
    /// 白名单成员的平台身份（`onebot.whitelist`）。
    pub(super) whitelist: &'a [String],
    /// 这个人是不是终端管理员（群里照投影，「施工时定的」第 183 条）。
    pub(super) admin: &'a dyn Fn(&ExternalId) -> bool,
}

/// 定下来的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Plan {
    /// 撤平台编号是它的那一条。
    Recall(String),
    /// 禁言这个人这么多秒（0 是解禁）。秒数大于 0 的，群主、群管理员还没挡（要问身份）。
    Mute {
        /// 禁谁。
        user: ExternalId,
        /// 几秒。
        seconds: u64,
    },
    /// 戳这个人。
    Poke(ExternalId),
    /// 不做，回一句。
    Refuse(Why),
}

/// 不做的为什么：每一种答一句（`tool-results/<名字>.txt`，「平台工具（一）」第 5 条）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Why {
    /// 不是管理的人叫她撤别人的、禁言。
    NotAllowed,
    /// 撤回：没引用，或者没有叫她做的那条。
    NoQuote,
    /// 禁言、戳一戳：认不出人。
    NoTarget,
    /// 禁言、戳一戳：不止一个人。
    ManyTargets,
    /// 禁言：动不得的人。
    Protected(ExternalId),
    /// 禁言：秒数没写、不是整数、出了范围。
    BadSeconds,
}

impl Why {
    /// 答的那一句的名字。
    pub(super) fn name(&self) -> &'static str {
        match self {
            Why::NotAllowed => "not-allowed",
            Why::NoQuote => "no-quote",
            Why::NoTarget => "no-target",
            Why::ManyTargets => "many-targets",
            Why::Protected(_) => "protected",
            Why::BadSeconds => "bad-seconds",
        }
    }
}

/// 定 `order` 做不做、对谁做：`caller` 叫的，叫她做的那条是 `origin`，场所的情形 `scene`。私聊里的禁言（`venues` 不给）由调的
/// 一方挡在前面。
pub(super) fn plan(order: Order, caller: &Caller, origin: Option<&Origin>, scene: &Scene) -> Plan {
    let origin = origin.filter(|origin| caller.id.as_ref() == Some(&origin.sender));
    match order {
        Order::Recall => recall(caller, origin, scene),
        Order::Mute(seconds) => mute(seconds, caller, origin, scene),
        Order::Poke => poke(caller, origin, scene),
    }
}

/// 撤回：引用的那一条；群里别人的要管理的人叫，私聊不分（交 QQ，第 180 条）。
fn recall(caller: &Caller, origin: Option<&Origin>, scene: &Scene) -> Plan {
    let Some(quote) = origin.and_then(|origin| origin.quote.as_ref()) else {
        return Plan::Refuse(Why::NoQuote);
    };
    if scene.peer.is_some() || quote.mine || caller.manager {
        Plan::Recall(quote.msg.clone())
    } else {
        Plan::Refuse(Why::NotAllowed)
    }
}

/// 禁言：先看谁叫的，再看秒数，再认人。
fn mute(seconds: Option<u64>, caller: &Caller, origin: Option<&Origin>, scene: &Scene) -> Plan {
    if !caller.manager {
        return Plan::Refuse(Why::NotAllowed);
    }
    let Some(seconds) = seconds else {
        return Plan::Refuse(Why::BadSeconds);
    };
    // 她的那一条没有发的人（她说的不是人说的话），去掉她的那一步在 `one` 里。
    let quoted = origin
        .and_then(|origin| origin.quote.as_ref())
        .and_then(|quote| quote.sender.as_ref());
    let mentions = origin
        .map(|origin| origin.mentions.as_slice())
        .unwrap_or_default();
    let user = match one(quoted.into_iter().chain(mentions), scene.me) {
        Ok(user) => user,
        Err(why) => return Plan::Refuse(why),
    };
    let protected = (scene.admin)(&user) || scene.whitelist.iter().any(|one| one == user.as_str());
    if seconds > 0 && protected {
        return Plan::Refuse(Why::Protected(user));
    }
    Plan::Mute { user, seconds }
}

/// 戳一戳：私聊戳对方；群里 @ 的那一个，没有的戳叫她的人。
fn poke(caller: &Caller, origin: Option<&Origin>, scene: &Scene) -> Plan {
    if let Some(peer) = scene.peer {
        return Plan::Poke(peer.clone());
    }
    let mentions = origin
        .map(|origin| origin.mentions.as_slice())
        .unwrap_or_default();
    match one(mentions, scene.me) {
        Ok(user) => Plan::Poke(user),
        Err(Why::NoTarget) => match &caller.id {
            Some(id) => Plan::Poke(id.clone()),
            None => Plan::Refuse(Why::NoTarget),
        },
        Err(why) => Plan::Refuse(why),
    }
}

/// 从 `people` 里挑出唯一的一个人：去掉她自己 `me`、重的；一个都没有是 [`Why::NoTarget`]，不止一个是 [`Why::ManyTargets`]。
fn one<'a>(
    people: impl IntoIterator<Item = &'a ExternalId>,
    me: &ExternalId,
) -> Result<ExternalId, Why> {
    let mut found: Vec<&ExternalId> = Vec::new();
    for person in people {
        if person != me && !found.contains(&person) {
            found.push(person);
        }
    }
    match found.as_slice() {
        [] => Err(Why::NoTarget),
        [only] => Ok((*only).clone()),
        _ => Err(Why::ManyTargets),
    }
}

/// 禁言多久，照人的单位写（「施工时定的」第 182 条）：天、时、分、秒各一段，`1d 2h 30m 5s`，0 的一段不写（0 秒是解禁，不用它）。
pub(super) fn duration(seconds: u64) -> String {
    let parts = [
        (seconds / 86_400, "d"),
        (seconds % 86_400 / 3_600, "h"),
        (seconds % 3_600 / 60, "m"),
        (seconds % 60, "s"),
    ];
    let written: Vec<String> = parts
        .iter()
        .filter(|(count, _)| *count > 0)
        .map(|(count, unit)| format!("{count}{unit}"))
        .collect();
    written.join(" ")
}

#[cfg(test)]
mod tests;

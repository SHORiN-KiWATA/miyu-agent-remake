//! 会话的事件（`docs/designs/03-事件模型.md` 第三节「会话与回合的事件怎么写」）。

use serde::{Deserialize, Serialize};

use crate::id::{AccountId, ContentHash, Seq, SessionId, VenueId};
use crate::text_enum::text_enum;

/// `session.created`：会话创建。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCreated {
    /// 会话的属主。默认只有属主能读、改；群的会话，属主是管理员（`06-多用户与身份.md` 第三节）。
    pub owner: AccountId,
    /// 会话所在的场所，例如 `local`、一个群 `qq:group:123456`。
    pub venue: VenueId,
    /// 策略快照（03 E5）。
    pub policy: ContentHash,
    /// 开始时的权限。
    pub permission: Permission,
    /// 一次性的：`miyu ask` 开的，`--continue`、`miyu undo` 不写会话时接的是最新的这种（`22-命令行.md` O2，
    /// 施工 3-9 下）。会话列表里折叠随终端界面那一步。不是一次性的不写。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub oneshot: bool,
    /// 开会话时实际干活的目录，人看到的那种写法（施工 4-9 再补三上）：还没开过回合的会话，核心重启以后照它载入。
    /// 之前的日志没有这一格。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// 父会话：派它的那个会话（施工 7-1，`agents.md`）。主会话没有；子会话不写 `oneshot`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<SessionId>,
    /// 第几层：父会话的加一，主会话是第 0 层、不写。和 `parent` 同有同无、至少是 1，由账本查（`kernel/history.md`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    /// 会话用哪个模型（施工 8-8，`models.md`「事件」）：造会话时解析好的引用，模型 `<供应商>/<模型>` 或池 `@<池>`（施工 8-8
    /// 造的可能是挡位换成的那时的值）。协议造的照 `session.create` 的 `model`，没写的照那时的 `models.chat`；子会话的照
    /// `subagent` 的 `pool`（施工 8-8 补；8-8 是 `tier`），没写的照父会话那时的。那时连 `models.chat` 都没配的不写；以前的日志没有这一格，照没有读。内核只记不解读，
    /// 引用怎么认在 `miyu-models`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// `session.policy_changed`：换了策略快照，或者换了权限，或者换了模型，也可以一起换。
/// 谁换的看事件的 `by`：人改的是配置，内核换的是目录变了、钉着的模型没了。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyChanged {
    /// 新的策略快照，下一个回合开始时生效（`02-内核.md` K3）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<ContentHash>,
    /// 新的权限：收紧当场生效，放宽下一步生效（`11-权限与沙盒.md` 第二节）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission: Option<Permission>,
    /// 换成的模型引用（施工 8-10，`models.md`「事件」）：模型 `<供应商>/<模型>` 或池 `@<池>`（施工 8-8 补以前，挡位在协议
    /// 那一头已经换成了它的值）。人换的 `by` 是人；钉着的没了、内核退回默认的 `by` 是内核，带着 `replaced`。下一个回合开始时生效，撤掉的回合里
    /// 的也算。以前的日志没有这一格。内核只记不解读。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// 钉着的引用没了、内核退回默认时写（施工 8-10）：原来那个。只和 `model` 一起出现（账本查）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaced: Option<String>,
}

/// `session.meta_changed`：改了哪项写哪项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaChanged {
    /// 新的标题。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// 置顶还是取消置顶。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<bool>,
}

/// `session.recapped`：一句回顾（施工 3-8 四补，`docs/blueprint/kernel/session.md`「回顾」）。头要的，推给所有订阅着的头，
/// 不进她的上下文；不带回合编号。`by` 是内核，`cause` 是要它的那个命令。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecapped {
    /// 那一句：她写的，去掉了前后空白，不是空的。
    pub text: String,
    /// 照到第几条：喂进回顾请求的最新那一条消息的序号，在这一条之前（账本查）。下一次要回顾时照到的还是它，交回这一句、
    /// 不再请求。
    pub upto: Seq,
}

/// 权限：常用的那一级，加上叠在上面的只读开关（`11-权限与沙盒.md` 第二节）。两格都写。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permission {
    /// 常用的那一级：工作区，或者完全放开。
    pub level: Level,
    /// 只读开关。开着的时候实际的级别就是只读，关掉回到常用的那一级。
    pub read_only: bool,
}

text_enum!(
    /// 常用的那一级。不认识的级别当什么，由用到它的地方决定：按最严的算。
    Level {
        /// 工作区：工作区里能读能写，边界以外要问人（`11-权限与沙盒.md` 第二节）。命令进沙盒随 M5。
        Workspace = "workspace",
        /// 完全放开：边界以内以外都放行，只挡数据根（`11-权限与沙盒.md` 第二节）。谁能切到这一级随多用户那一步。
        Full = "full",
    }
);

#[cfg(test)]
mod tests;

//! 核心推来的提问、确认，和作答交回去的形状（核心 D-1 `session.answer`、D-2 `ask_user`）（`docs/designs/samples/events/` 的 `question.asked`、
//! `tool.approval_requested`、`question.answered`、`tool.approval_decided`）。
//!
//! 读的时候不拒认不得的字段：核心加了字段，头照旧能读。

use serde::{Deserialize, Serialize};

/// `question.asked` 的 `body`。
#[derive(Debug, Clone, Deserialize)]
pub struct Asked {
    /// 这次调用的编号：作答照它交回去。
    pub call_id: String,
    /// 一道道题。
    pub questions: Vec<Question>,
}

/// 一道题。
#[derive(Debug, Clone, Deserialize)]
pub struct Question {
    /// 标签上写的短名；没有的用「第 N 题」。
    #[serde(default)]
    pub header: Option<String>,
    /// 问题。
    pub question: String,
    /// 选项；没有的只能自己写。
    #[serde(default)]
    pub options: Vec<Choice>,
    /// 能不能多选。
    #[serde(default)]
    pub multiple: bool,
}

/// 一个选项。
#[derive(Debug, Clone, Deserialize)]
pub struct Choice {
    /// 标题。
    pub label: String,
    /// 一行说明。
    #[serde(default)]
    pub description: Option<String>,
    /// 一段文字画：抽屉里画在选项右边（照 Claude Code；协议还没有，已转告加上）。
    #[serde(default)]
    pub preview: Option<String>,
}

/// `tool.approval_requested` 的 `body`。
#[derive(Debug, Clone, Deserialize)]
pub struct Approval {
    /// 这次调用的编号。
    pub call_id: String,
    /// 要什么权限：`write`、`read`、`exec` 这类。
    pub access: String,
    /// 提的放行规则：有的才能选「这个会话都允许」（核心 D-1：没有的选了回 `no_rule`）。
    #[serde(default)]
    pub rule: Option<serde_json::Value>,
    /// 碰到哪些东西。
    #[serde(default)]
    pub detail: Option<Detail>,
}

/// 确认的细节。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Detail {
    /// 碰到的路径。
    #[serde(default)]
    pub paths: Vec<Touched>,
    /// 哪件工具。
    #[serde(default)]
    pub tool: Option<String>,
}

/// 碰到的一个路径。
#[derive(Debug, Clone, Deserialize)]
pub struct Touched {
    /// 路径。
    pub path: String,
    /// 在哪：`outside` 是工作区外。
    #[serde(default)]
    pub zone: Option<String>,
}

/// `question.answered` 的 `body`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answered {
    /// 调用编号。
    pub call_id: String,
    /// 一道一个回答，照题的先后。
    pub answers: Vec<Answer>,
}

/// 一道题的回答：选了哪几项、自己写的话。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answer {
    /// 选中的选项的标题。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub picked: Vec<String>,
    /// 自己写的话（「其他」）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 补充的话（按 `n` 写的；核心 D-2 起有）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// `tool.approval_decided` 的 `body`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decided {
    /// 调用编号。
    pub call_id: String,
    /// 怎么定的。
    pub decision: Decision,
    /// 不允许时写的理由。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// 确认的几种回答（核心 D-1；「这个工作区都允许」核心还没有）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decision {
    /// 允许这一次。
    Once,
    /// 这个会话都允许。
    Session,
    /// 不允许。
    Deny,
}

impl Decision {
    /// 抽屉里的先后。
    pub const ALL: [Self; 3] = [Self::Once, Self::Session, Self::Deny];
}

/// 演示用的一条假事件：谁在问（子代理、后台命令问的才有），和核心推的 `body` 一个形状。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fake<T> {
    /// 谁在问：`子代理 查资料` 这样；她自己问的不写。
    #[serde(default)]
    pub who: Option<String>,
    /// 事件的 `body`。
    pub body: T,
}

//! 给人看的字由外面交进来（这里不读文件）：工具的显示名和对象、说法换成字。端点照资源目录的 `human/<语言>.json`
//! 交一份连接的语言的、一份英文的；工具算哪一类照 `core/view.json`。

use std::collections::BTreeMap;

use serde::Deserialize;

use miyu_kernel::event::Said;

/// 一种语言的字。
pub trait Words {
    /// 工具 `name` 的显示名，和它的对象取哪一个参数；没有的是 `None`。
    fn face(&self, name: &str) -> Option<Face>;

    /// 编号是 `key` 的说法换成字，字段照 `fields` 换；没有这一句的是 `None`。
    fn say(&self, said: &Said) -> Option<String>;
}

/// 一件工具给人看的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Face {
    /// 显示名。
    pub name: String,
    /// 对象取哪一个参数。
    pub subject: Option<String>,
}

/// 工具算哪一类：收起那一行照它数，标题照它写。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    /// 执行命令。
    Command,
    /// 编辑、写入。
    Edit,
    /// 派子代理。
    Agent,
    /// 留言。
    Message,
}

/// `core/view.json`：语言之外的那几样。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kinds {
    /// 工具名到它算哪一类；不在里面的算别的工具。
    #[serde(default)]
    pub kinds: BTreeMap<String, ToolKind>,
    /// 参数里哪一格是别的会话的编号。
    #[serde(default)]
    pub session_arg: Option<String>,
}

impl Kinds {
    /// 工具 `name` 算哪一类。
    #[must_use]
    pub fn of(&self, name: &str) -> Option<ToolKind> {
        self.kinds.get(name).copied()
    }
}

/// 投影要的全部字：连接的语言一份，英文一份（收起那一行另给英文），工具的分类，家目录。
pub struct Texts {
    /// 连接的语言。
    pub local: Box<dyn Words + Send + Sync>,
    /// 英文。
    pub english: Box<dyn Words + Send + Sync>,
    /// 工具的分类。
    pub kinds: Kinds,
    /// 核心这台机器的家目录：对象里的路径在它底下的写成 `~/…`。
    pub home: Option<String>,
}

/// 说法的编号，都在内核那一份的 `view/` 下面。
pub(crate) mod keys {
    pub const PREPARE: &str = "core/view/prepare";
    pub const ON_SESSION: &str = "core/view/on-session";
    pub const PARENT: &str = "core/view/parent";
    pub const THOUGHT_FOR: &str = "core/view/summary/thought-for";
    pub const REASON_WITH: &str = "core/view/error/with";
    pub const CLASS: &str = "core/view/error/class";
    pub const STATUS: &str = "core/view/error/status";
    pub const SUMMARY: &str = "core/view/summary";
}

/// 照编号和字段说一句。
pub(crate) fn say(words: &dyn Words, key: &str, fields: &[(&str, String)]) -> Option<String> {
    let mut said = Said::new(key);
    for (name, value) in fields {
        said.fields.insert((*name).to_string(), value.clone());
    }
    words.say(&said)
}

/// 数是 `count` 的一句：是 1 的编号多接 `/one`（`human.rs`「一个的时候说单数」）。
pub(crate) fn count(words: &dyn Words, key: &str, count: usize) -> String {
    let key = match count {
        1 => format!("{key}/one"),
        _ => key.to_string(),
    };
    say(words, &key, &[("count", count.to_string())]).unwrap_or_else(|| count.to_string())
}

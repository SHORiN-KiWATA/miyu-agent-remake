//! 改标题、置顶的参数（施工 3-8 三补，`docs/blueprint/protocol.md` 的 `session.set_meta`）：标题去掉前后空白，1 到
//! 200 个字；写 `null` 是去掉标题，记成空的标题（`kernel/events-bodies.md` 的 `session.meta_changed`）；两格都不写的是
//! 参数不对。

use serde::{Deserialize, Deserializer};

use miyu_kernel::session::Command;

use crate::refusal::Refusal;

/// 标题最多几个字（Unicode 字符），去掉前后空白以后数。
const TITLE_CHARS: usize = 200;

/// `session.set_meta` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct MetaParams {
    /// 哪个会话。
    pub(crate) session: String,
    /// 新的标题：没写是不改，写 `null` 是去掉，别的照写的。和别的「可以不写」的格不一样，`null` 有意思，所以记着写没写。
    #[serde(default, deserialize_with = "written")]
    title: Option<Option<String>>,
    /// 置顶还是取消置顶；写 `null` 和没写一样。
    #[serde(default)]
    pinned: Option<bool>,
}

impl MetaParams {
    /// 换成交给会话的命令：标题去掉前后空白、量过长短，去掉标题的写成空的。两格都不写（`pinned` 写 `null` 也算没写）、标题
    /// 去掉空白以后是空的、超过 200 个字的，是参数不对。
    pub(crate) fn command(&self) -> Result<Command, Refusal> {
        if self.title.is_none() && self.pinned.is_none() {
            return Err(Refusal::BAD_PARAMS);
        }
        let title = match &self.title {
            None => None,
            Some(None) => Some(String::new()),
            Some(Some(title)) => Some(trimmed(title)?),
        };
        Ok(Command::SetMeta {
            title,
            pinned: self.pinned,
        })
    }
}

/// 写了这一格：`null` 读成 `Some(None)`，没写的由 `default` 读成 `None`。
fn written<'de, D: Deserializer<'de>>(read: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(read).map(Some)
}

/// 去掉前后空白的标题：空的、太长的是参数不对。
fn trimmed(title: &str) -> Result<String, Refusal> {
    let title = title.trim();
    match title.chars().count() {
        1..=TITLE_CHARS => Ok(title.to_string()),
        _ => Err(Refusal::BAD_PARAMS),
    }
}

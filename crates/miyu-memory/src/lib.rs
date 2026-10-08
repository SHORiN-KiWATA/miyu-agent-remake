//! 记忆这个软件包（施工 R-3 中，`docs/blueprint/memory.md`「工具」）：她用来碰自己的记忆的三件工具。不放进基础系统：记忆是
//! 可选、能关的软件包（`docs/designs/10-自带软件.md` 第三节）。
//!
//! - [`remember`]：记一条，或者改一条（`replaces`）；
//! - [`forget`]：作废一条；
//! - [`memory_search`]：搜记下的和以前的对话。
//!
//! 工具只拿 [`miyu_tool::MemoryPort`]：记忆日志、检索库、会话、听众都在端口后面（实现在 `miyu-session`）。给模型看的字在
//! 资源目录的 `software/memory/` 下（进登记簿），给人看的在 `software/memory/human/`。
//!
//! [`remember`]: miyu_tool::REMEMBER
//! [`forget`]: miyu_tool::FORGET
//! [`memory_search`]: miyu_tool::MEMORY_SEARCH

mod forget;
mod remember;
mod search;
mod texts;

use std::path::Path;
use std::sync::Arc;

use miyu_tool::Tool;
use miyu_tool::load::LoadError;

/// 这个软件包的编号：资源目录里的名字，也是预设里开关它的那个键（施工 P-2 中）。
pub const PACKAGE: &str = "memory";

/// 一条记忆最多几个字：和协议、斜杠命令记的照同一个数，住在 `miyu-tool`（施工 R-3 补）。
pub use miyu_tool::TEXT_CHARS;

/// 一段以前的对话最多给几个字，截了的末尾接 `…`：要整段她用 `history` 带 `session` 去读。
pub const TURN_CHARS: usize = 300;

/// 三件工具，照 `remember`、`forget`、`memory_search` 的先后。字从资源目录 `resources` 读。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对。
pub fn tools(resources: &Path) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    let texts = texts::Texts::load(resources)?;
    Ok(vec![
        Arc::new(remember::Remember::load(resources, texts.clone())?),
        Arc::new(forget::Forget::load(resources, texts.clone())?),
        Arc::new(search::Search::load(resources, texts)?),
    ])
}

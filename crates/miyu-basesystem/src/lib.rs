//! 基础系统（`docs/designs/10-自带软件.md` 第三节）：随发行附带、几乎每个预设都打开的几件工具，软件包的编号
//! 是 `basesystem`。
//!
//! 工具的说明、参数格式、输出里给她看的几句都放在资源目录的 `software/basesystem/` 下（`26-提示词.md`
//! 第八节），核心起来时读。现在 [`tools`] 里有读的三件：`read`（施工 4-4 上）、`glob`、`grep`（施工 4-4 下），
//! 和写的 `write`（施工 4-6 上）、`edit`（施工 4-6 中）、`trash`（施工 4-6 下），执行命令的 `shell`（施工 4-8），
//! 翻这个会话自己记录的 `history`（施工 6-4，施工 C-4 多认别的会话），派子代理的 `subagent`（施工 7-5，7-5 再补从 `agent`
//! 改名），看、停派出去的任务的 `jobs`（施工 7-4），发话的 `send_message`（施工 7-7，施工 C-5 从 `message_agent` 改名，
//! 多发得到别的会话），列你别的主会话的 `sessions`（施工 C-3），查这个会话的用量、金额、上下文的 `session_usage`（施工 8-15）。
//! 名字、参数、输出照成熟 harness 的规范，以 Claude Code 为主（`10-自带软件.md` 第十节）。

mod blocking;
mod common;
mod edit;
mod glob;
mod grep;
mod history;
mod jobs;
mod load;
mod pattern;
mod read;
mod send_message;
mod session_usage;
mod sessions;
mod shell;
mod subagent;
mod text;
mod trash;
mod walk;
mod write;

use std::path::Path;
use std::sync::Arc;

use miyu_tool::Tool;

pub use load::LoadError;

/// 基础系统的每件工具：照资源目录 `resources` 里的字造好。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对，说是哪一份。
pub fn tools(resources: &Path) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    let common = common::Common::load(resources)?;
    Ok(vec![
        Arc::new(read::Read::load(resources, common.clone())?),
        Arc::new(glob::Glob::load(resources, common.clone())?),
        Arc::new(grep::Grep::load(resources, common.clone())?),
        Arc::new(write::Write::load(resources, common.clone())?),
        Arc::new(edit::Edit::load(resources, common.clone())?),
        Arc::new(trash::Trash::load(resources, common.clone())?),
        Arc::new(shell::Shell::load(resources, common.clone())?),
        Arc::new(history::History::load(resources, common.clone())?),
        Arc::new(subagent::Subagent::load(resources, common.clone())?),
        Arc::new(jobs::Jobs::load(resources, common.clone())?),
        Arc::new(send_message::SendMessage::load(resources, common.clone())?),
        Arc::new(sessions::Sessions::load(resources, common)?),
        Arc::new(session_usage::SessionUsage::load(resources)?),
    ])
}

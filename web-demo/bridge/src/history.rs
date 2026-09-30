//! 只读地读会话日志：`/blob` 照它确认这个 blob 在这个会话里出现过（`media.rs`）。读历史本身 2026-10-01 起用核心的订阅补发
//! （`subscribe` 的 `after`，施工 3-8 六补），桥不再顶替 `events.read`。
//!
//! 照 `miyu-store` 的 `read_events`：和核心一样自检，最后一行写了一半的跳过、不截，一个字节都不写（会话可能
//! 正在往里写）。每一条照 `Event::to_line` 写回原样的 JSON，和推送里的 `event` 同一个样子。
//! 核心有了给 blob 的地址（网页模块）以后删掉这一份。

use serde_json::Value;

use miyu_kernel::id::{AccountId, SessionId};
use miyu_store::root::DataRoot;

/// 一个会话里序号大于 `after` 的全部事件。
///
/// # Errors
///
/// 账号、会话编号不合写法；没有这个会话；日志坏了：交回一句说清楚的话。
pub fn read(root: &DataRoot, account: &str, session: &str, after: u64) -> Result<Vec<Value>, String> {
    let account = AccountId::parse(account).map_err(|e| format!("账号 {account}：{e}"))?;
    let session = SessionId::parse(session).map_err(|e| format!("会话编号 {session}：{e}"))?;
    let dir = root.session_dir(&account, &session);
    let events = miyu_store::log::read_events(&dir).map_err(|e| format!("读不了会话日志：{e}"))?;
    events
        .iter()
        .filter(|e| e.seq.get() > after)
        .map(|e| serde_json::from_str(&e.to_line()).map_err(|e| format!("写回 JSON 时出错：{e}")))
        .collect()
}

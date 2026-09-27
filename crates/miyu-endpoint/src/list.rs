//! 列出会话（`docs/designs/04-核心协议.md` 第五节「会话列表」的第一小块，施工 3-9 下）：管理员的会话，从新到
//! 旧；`oneshot` 的只要一次性的。读每个会话日志的第一条，只读不写，不碰正在写的最后一段。以后有了会话
//! 列表流、索引再换。

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_kernel::id::AccountId;
use miyu_store::log::{OpenError, first_event};
use miyu_store::root::DataRoot;

use crate::Core;
use crate::refusal::Refusal;

/// 管理员的会话，从新到旧，最多 `limit` 个：`[{session, oneshot}]`。
pub(crate) async fn list(
    core: &Core,
    oneshot: bool,
    limit: Option<usize>,
) -> Result<Vec<Value>, Refusal> {
    let root = core.root.clone();
    let admin = core.admin.clone();
    match tokio::task::spawn_blocking(move || scan(&root, &admin, oneshot, limit)).await {
        Ok(listed) => listed,
        Err(error) => {
            tracing::error!(target: "miyu::endpoint", error = %error, "list panicked");
            Err(Refusal::INTERNAL)
        }
    }
}

/// 在阻塞线程里一个个读第一条。
fn scan(
    root: &DataRoot,
    account: &AccountId,
    oneshot: bool,
    limit: Option<usize>,
) -> Result<Vec<Value>, Refusal> {
    let ids = root.sessions(account).map_err(|error| {
        tracing::warn!(target: "miyu::endpoint", error = %error, "sessions not listed");
        Refusal::INTERNAL
    })?;
    let mut found = Vec::new();
    for id in ids {
        if limit.is_some_and(|limit| found.len() >= limit) {
            break;
        }
        let created = match first_event(&root.session_dir(account, &id)) {
            Ok(event) => match event.body {
                Body::SessionCreated(created) => created,
                _ => continue,
            },
            Err(OpenError::Missing(_)) => continue,
            Err(error) => {
                tracing::warn!(target: "miyu::endpoint", session = id.as_str(), error = %error, "first event not read");
                continue;
            }
        };
        if oneshot && !created.oneshot {
            continue;
        }
        found.push(json!({"session": id.as_str(), "oneshot": created.oneshot}));
    }
    Ok(found)
}

//! 列出会话（`docs/designs/04-核心协议.md` 第五节「会话列表」的第一小块，施工 3-9 下）：管理员的会话，从新到
//! 旧；`oneshot` 的只要一次性的。读每个会话日志的第一条，只读不写，不碰正在写的最后一段；列进去的再整份读一遍，照
//! `session.meta_changed` 算出标题、置顶（施工 3-8 三补）。以后有了会话列表流、索引再换：现在每列一次，列进去的会话
//! 日志都整份读一遍。

use std::path::Path;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_kernel::id::{AccountId, SessionId};
use miyu_store::log::{OpenError, first_event, read_segments};
use miyu_store::root::DataRoot;

use crate::Core;
use crate::refusal::Refusal;

/// 管理员的会话，从新到旧，最多 `limit` 个：`[{session, oneshot, parent, title?, pinned?}]`。
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
        // 父会话（施工 7-5）：子会话写它的编号，主会话写 `null`。
        let mut item =
            json!({"session": id.as_str(), "oneshot": created.oneshot, "parent": created.parent});
        let (title, pinned) = meta(&root.session_dir(account, &id), &id);
        if !title.is_empty() {
            item["title"] = json!(title);
        }
        if pinned {
            item["pinned"] = json!(true);
        }
        found.push(item);
    }
    Ok(found)
}

/// 会话目录 `dir` 的日志里最后改成的标题、置顶（施工 3-8 三补）：`session.meta_changed` 一条条盖上去，标题空的是没有。读不
/// 下去的（日志坏了）记一行，照坏的那一段以前的算：一段查过了才交出来。
fn meta(dir: &Path, id: &SessionId) -> (String, bool) {
    let (mut title, mut pinned) = (String::new(), false);
    let read = read_segments(dir, |events| {
        for event in events {
            if let Body::MetaChanged(changed) = event.body {
                if let Some(new) = changed.title {
                    title = new;
                }
                pinned = changed.pinned.unwrap_or(pinned);
            }
        }
        true
    });
    if let Err(error) = read {
        tracing::warn!(target: "miyu::endpoint", session = id.as_str(), error = %error, "meta not read");
    }
    (title, pinned)
}

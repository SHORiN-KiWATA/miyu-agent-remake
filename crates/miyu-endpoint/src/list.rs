//! 列出会话（`docs/designs/04-核心协议.md` 第五节「会话列表」的第一小块，施工 3-9 下）：管理员的会话，从新到
//! 旧；`oneshot` 的只要一次性的。读每个会话日志的第一条，只读不写，不碰正在写的最后一段；列进去的再整份读一遍，照
//! `session.meta_changed` 算出标题、置顶（施工 3-8 三补），再算工作目录、最近一次动静，看会话表里它忙不忙（施工 C-3）。
//! 以后有了会话列表流、索引再换：现在每列一次，列进去的会话日志都整份读一遍。
//!
//! 她用 `sessions` 列会话也是这一个函数（[`scan`]，`cross-session.md` 第一条第 2 款），经会话表交给会话的端口
//! （`crate::spawn`）：头和她看到的是同一份。

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};

use miyu_kernel::event::{Body, Event, SessionCreated};
use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::time::Timestamp;
use miyu_store::log::{OpenError, first_event, read_segments};
use miyu_store::root::DataRoot;
use miyu_tool::Stop;

use crate::Core;
use crate::refusal::Refusal;

/// 日志里一条工作目录都没记的（很早以前的日志）照这个算：当头报来的是 `~`（`protocol.md`「会话表」第 5 条）。
pub(crate) const NO_CWD: &str = "~";

/// 管理员的会话，从新到旧，最多 `limit` 个：`[{busy?, cwd, last_active, oneshot, parent, pinned?, session, title?}]`。
pub(crate) async fn list(
    core: &Core,
    oneshot: bool,
    limit: Option<usize>,
) -> Result<Vec<Value>, Refusal> {
    let root = core.root.clone();
    let admin = core.admin.clone();
    let busy = core.sessions.busy_ids().await;
    let pick = move |created: &SessionCreated| !oneshot || created.oneshot;
    let scanned = tokio::task::spawn_blocking(move || {
        scan(&root, &admin, &busy, pick, limit, &Stop::default())
    })
    .await;
    match scanned {
        Ok(Ok(listed)) => Ok(listed.iter().map(Listed::to_json).collect()),
        Ok(Err(error)) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "sessions not listed");
            Err(Refusal::INTERNAL)
        }
        Err(error) => {
            tracing::error!(target: "miyu::endpoint", error = %error, "list panicked");
            Err(Refusal::INTERNAL)
        }
    }
}

/// 列出来的一个会话：`session.created` 里的几样，照整份日志算的几样，会话表里忙不忙。
#[derive(Debug)]
pub(crate) struct Listed {
    pub(crate) id: SessionId,
    pub(crate) oneshot: bool,
    /// 父会话（施工 7-5）：子会话才有。
    pub(crate) parent: Option<SessionId>,
    /// 标题：空的是没有。
    pub(crate) title: String,
    pub(crate) pinned: bool,
    /// 工作目录（施工 C-3）：头报来的写法。
    pub(crate) cwd: String,
    /// 这时有回合在进行（施工 C-3）。
    pub(crate) busy: bool,
    /// 日志最后一条事件的时刻（施工 C-3）。
    pub(crate) last_active: Timestamp,
}

impl Listed {
    /// `session.list` 的一项：有标题的才写 `title`，置顶的、忙的才写 `pinned`、`busy`（写 `true`）。
    fn to_json(&self) -> Value {
        let mut item = json!({
            "session": self.id.as_str(),
            "oneshot": self.oneshot,
            "parent": self.parent,
            "cwd": self.cwd,
            "last_active": self.last_active,
        });
        if !self.title.is_empty() {
            item["title"] = json!(self.title);
        }
        if self.pinned {
            item["pinned"] = json!(true);
        }
        if self.busy {
            item["busy"] = json!(true);
        }
        item
    }
}

/// 在阻塞线程里一个个读：账号 `account` 的会话从新到旧，`session.created` 合 `pick` 的列进去，最多 `limit` 个；`busy` 里的
/// 是忙的。读下一个会话之前看一眼 `stop`，举起来了交回已经读到的。
///
/// # Errors
///
/// 读不了放会话的目录。
pub(crate) fn scan(
    root: &DataRoot,
    account: &AccountId,
    busy: &BTreeSet<SessionId>,
    pick: impl Fn(&SessionCreated) -> bool,
    limit: Option<usize>,
    stop: &Stop,
) -> std::io::Result<Vec<Listed>> {
    let mut found = Vec::new();
    for id in root.sessions(account)? {
        if stop.stopped() || limit.is_some_and(|limit| found.len() >= limit) {
            break;
        }
        let dir = root.session_dir(account, &id);
        let (created, at) = match first_event(&dir) {
            Ok(event) => match event.body {
                Body::SessionCreated(created) => (created, event.at),
                _ => continue,
            },
            Err(OpenError::Missing(_)) => continue,
            Err(error) => {
                tracing::warn!(target: "miyu::endpoint", session = id.as_str(), error = %error, "first event not read");
                continue;
            }
        };
        if !pick(&created) {
            continue;
        }
        let mut read = Read::new(&created, at);
        read.log(&dir, &id);
        found.push(Listed {
            busy: busy.contains(&id),
            id,
            oneshot: created.oneshot,
            parent: created.parent,
            title: read.title,
            pinned: read.pinned,
            cwd: read.cwd.unwrap_or_else(|| NO_CWD.to_string()),
            last_active: read.last,
        });
    }
    Ok(found)
}

/// 照整份日志算的几样。
struct Read {
    title: String,
    pinned: bool,
    cwd: Option<String>,
    last: Timestamp,
}

impl Read {
    /// 从第一条算起：`session.created` 的工作目录，它的时刻。
    fn new(created: &SessionCreated, at: Timestamp) -> Read {
        Read {
            title: String::new(),
            pinned: false,
            cwd: created.cwd.clone(),
            last: at,
        }
    }

    /// 会话目录 `dir` 的日志从头读一遍（施工 3-8 三补、C-3）：`session.meta_changed` 一条条盖上标题、置顶，标题空的是没有；
    /// 带 `cwd` 的一条条盖上工作目录；最后一条的时刻。读不下去的（日志坏了）记一行，照坏的那一段以前的算：一段查过了才
    /// 交出来。
    fn log(&mut self, dir: &Path, id: &SessionId) {
        let read = read_segments(dir, |events| {
            for event in &events {
                self.see(event);
            }
            true
        });
        if let Err(error) = read {
            tracing::warn!(target: "miyu::endpoint", session = id.as_str(), error = %error, "meta not read");
        }
    }

    /// 照先后看一条。
    fn see(&mut self, event: &Event) {
        self.last = event.at;
        if let Some(cwd) = cwd(event) {
            self.cwd = Some(cwd.to_string());
        }
        if let Body::MetaChanged(changed) = &event.body {
            if let Some(new) = &changed.title {
                self.title.clone_from(new);
            }
            self.pinned = changed.pinned.unwrap_or(self.pinned);
        }
    }
}

/// 这一条记下的工作目录（`protocol.md`「会话表」第 5 条）：带 `cwd` 的 `turn.started`、`session.created`。会话表载入时、列会话时
/// 都照日志里最后一条带它的算，同一个认法。
pub(crate) fn cwd(event: &Event) -> Option<&str> {
    match &event.body {
        Body::TurnStarted(started) => started.cwd.as_deref(),
        Body::SessionCreated(created) => created.cwd.as_deref(),
        _ => None,
    }
}

#[cfg(test)]
mod tests;

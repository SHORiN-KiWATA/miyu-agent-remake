//! 列出会话（`docs/designs/04-核心协议.md` 第五节「会话列表」的第一小块，施工 3-9 下）：管理员的会话，从新到旧；`oneshot`
//! 的只要一次性的。标题、置顶（施工 3-8 三补），工作目录、最近一次动静（施工 C-3）照日志算，忙不忙看会话表。
//!
//! 照日志算的几样读会话列表的索引（施工 3-8 七补，`docs/blueprint/store/index.md`）：一个会话一行，记着照到日志的哪里。
//! 照到的就是日志现在的末尾的，直接用；日志比它长的只读多出来的那一截，补好写回去；没有这一行、对不上的（日志比记的
//! 短了、段对不上），这一个会话整份读一遍，和没有索引时一样。所以结果和每次整份读的一字不差，只是快。
//!
//! 她用 `sessions` 列会话也是这一个函数（[`scan`]，`cross-session.md` 第一条第 2 款），经会话表交给会话的端口
//! （`crate::spawn`）：头和她看到的是同一份。

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};

use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::time::Timestamp;
use miyu_store::index::{FILE, Opened, Row, SessionIndex};
use miyu_store::log::{Mark, OpenError, first_event, read_marked};
use miyu_store::root::DataRoot;
use miyu_tool::Stop;

use crate::Core;
use crate::refusal::Refusal;

pub(crate) use miyu_store::index::cwd;

/// 日志里一条工作目录都没记的（很早以前的日志）照这个算：当头报来的是 `~`（`protocol.md`「会话表」第 5 条）。
pub(crate) const NO_CWD: &str = "~";

/// 本机的场所。通讯平台的场所会话（施工 O-3，`venues.md`「不在本机的头上」）不进会话列表、会话列表的推送、`sessions` 工具。
pub(crate) const LOCAL: &str = "local";

/// 这一行是不是本机的会话。
pub(crate) fn local(row: &Row) -> bool {
    row.venue == LOCAL
}

/// 管理员的会话，从新到旧，最多 `limit` 个：`[{busy?, cwd, last_active, oneshot, parent, pinned?, session, title?}]`。
pub(crate) async fn list(
    core: &Core,
    oneshot: bool,
    limit: Option<usize>,
) -> Result<Vec<Value>, Refusal> {
    let root = core.root.clone();
    let admin = core.admin.clone();
    let index = core.index.clone();
    let busy = core.sessions.busy_ids().await;
    let pick = move |row: &Row| local(row) && (!oneshot || row.oneshot);
    let scanned = tokio::task::spawn_blocking(move || {
        scan(
            &root,
            &admin,
            Some(&index),
            &busy,
            pick,
            limit,
            &Stop::default(),
        )
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

/// 列出来的一个会话：`session.created` 里的几样，照日志算的几样，会话表里忙不忙。
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
    /// 第一句话的第一行（施工 9-5）：空的是没有。
    pub(crate) preview: String,
    /// 场所（施工 O-3）：本机的是 [`LOCAL`]。
    pub(crate) venue: String,
    /// 人格（施工 P-1 下）：以前的日志没有的是空的。
    pub(crate) persona: Option<String>,
}

impl Listed {
    /// 照索引的一行、会话表里忙不忙。
    fn new(row: Row, busy: bool) -> Listed {
        Listed {
            id: row.id,
            oneshot: row.oneshot,
            parent: row.parent,
            title: row.title,
            pinned: row.pinned,
            cwd: row.cwd.unwrap_or_else(|| NO_CWD.to_string()),
            busy,
            last_active: row.last_active,
            preview: row.preview,
            venue: row.venue,
            persona: row.persona,
        }
    }

    /// `session.list` 的一项：有标题的才写 `title`，没标题、说过话的写 `preview`（施工 9-5），置顶的、忙的才写 `pinned`、`busy`
    /// （写 `true`）。
    pub(crate) fn to_json(&self) -> Value {
        let mut item = json!({
            "session": self.id.as_str(),
            "oneshot": self.oneshot,
            "parent": self.parent,
            "cwd": self.cwd,
            "last_active": self.last_active,
        });
        if !self.title.is_empty() {
            item["title"] = json!(self.title);
        } else if !self.preview.is_empty() {
            item["preview"] = json!(self.preview);
        }
        if self.pinned {
            item["pinned"] = json!(true);
        }
        if self.busy {
            item["busy"] = json!(true);
        }
        if let Some(persona) = &self.persona {
            item["persona"] = json!(persona);
        }
        item
    }
}

/// 在阻塞线程里一个个读：账号 `account` 的会话从新到旧，合 `pick` 的列进去，最多 `limit` 个；`busy` 里的是忙的。读下一个
/// 会话之前看一眼 `stop`，举起来了交回已经读到的。`index` 是这个账号的会话列表的索引，没有的每个都整份读。
///
/// # Errors
///
/// 读不了放会话的目录。
pub(crate) fn scan(
    root: &DataRoot,
    account: &AccountId,
    index: Option<&SessionIndex>,
    busy: &BTreeSet<SessionId>,
    pick: impl Fn(&Row) -> bool,
    limit: Option<usize>,
    stop: &Stop,
) -> std::io::Result<Vec<Listed>> {
    let mut rows = match index.map(SessionIndex::rows) {
        Some(Ok(rows)) => rows,
        Some(Err(error)) => {
            // 用着用着坏了的：删掉重建，这一次整份读，读完的照样写进新的里。
            tracing::warn!(target: "miyu::endpoint", error = %error, "session index not read");
            if let Some(Err(error)) = index.map(SessionIndex::reset) {
                tracing::warn!(target: "miyu::endpoint", error = %error, "session index unusable");
            }
            Default::default()
        }
        None => Default::default(),
    };
    let mut found = Vec::new();
    for id in root.sessions(account)? {
        if stop.stopped() || limit.is_some_and(|limit| found.len() >= limit) {
            break;
        }
        let dir = root.session_dir(account, &id);
        let row = match rows.remove(&id) {
            // 属主、父会话、是不是一次性的只在 `session.created` 里，不会变：照旧的那一行挑。
            Some(row) if !pick(&row) => continue,
            Some(row) => match caught_up(&dir, &row, index) {
                Some(row) => Some(row),
                None => whole(&dir, id.clone(), Some(&row.mark), index, &pick),
            },
            None => whole(&dir, id.clone(), None, index, &pick),
        };
        if let Some(row) = row {
            found.push(Listed::new(row, busy.contains(&id)));
        }
    }
    Ok(found)
}

/// 只算会话 `id` 的一项（施工 9-5，会话列表的推送）：照索引的那一行补到日志现在的末尾，没有那一行、对不上的整份读，和
/// [`scan`] 里一个会话的算法一样；`busy` 是它这时忙不忙。会话目录没了（删了、挪进回收处）、没有日志、第一条读不出来的交回空。
pub(crate) fn one(
    root: &DataRoot,
    account: &AccountId,
    index: &SessionIndex,
    id: &SessionId,
    busy: bool,
) -> Option<Listed> {
    let dir = root.session_dir(account, id);
    if !dir.is_dir() {
        return None;
    }
    let all = |_: &Row| true;
    let row = match index.rows().ok().and_then(|mut rows| rows.remove(id)) {
        Some(row) => caught_up(&dir, &row, Some(index))
            .or_else(|| whole(&dir, id.clone(), Some(&row.mark), Some(index), &all)),
        None => whole(&dir, id.clone(), None, Some(index), &all),
    }?;
    Some(Listed::new(row, busy))
}

/// 索引里的一行补到日志现在的末尾（施工 3-8 七补）：只读它照到的地方后面多出来的那一截，一条条盖上去；多出来了的写回去，
/// 只换照到的还是原来那里的（会话自己同时盖过了的，照它的）。对不上的、后面读不下去的交回空的：这一个会话整份重读。
fn caught_up(dir: &Path, row: &Row, index: Option<&SessionIndex>) -> Option<Row> {
    let mut caught = row.clone();
    let end = read_marked(dir, Some(&row.mark), |events| {
        for event in &events {
            caught.see(event);
        }
        true
    });
    match end {
        Ok(Some(end)) => {
            if end != row.mark {
                caught.mark = end;
                put(index, &caught, Some(&row.mark));
            }
            Some(caught)
        }
        Ok(None) | Err(_) => None,
    }
}

/// 整份读一个会话的日志（没有索引时的读法，`protocol.md` 的 `session.list` 第 2 到 4 条）：先读第一条，不是
/// `session.created` 的、没有日志的、读不出来的（记一行）不列，不合 `pick` 的不列；再从头读一遍，一条条盖上去。读完了的
/// 写进索引：`was` 是空的，表里还没有这一行才写；不是空的，照到的还是它才换。后面读不下去的（日志坏了）记一行，照坏的那一段
/// 以前的算，不写进索引：一段查过了才交出来。
fn whole(
    dir: &Path,
    id: SessionId,
    was: Option<&Mark>,
    index: Option<&SessionIndex>,
    pick: &impl Fn(&Row) -> bool,
) -> Option<Row> {
    let mut row = match first_event(dir) {
        Ok(event) => Row::new(id, &event)?,
        Err(OpenError::Missing(_)) => return None,
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", session = id.as_str(), error = %error, "first event not read");
            return None;
        }
    };
    if !pick(&row) {
        return None;
    }
    let end = read_marked(dir, None, |events| {
        for event in &events {
            row.see(event);
        }
        true
    });
    match end {
        Ok(Some(end)) => {
            row.mark = end;
            put(index, &row, was);
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", session = row.id.as_str(), error = %error, "meta not read");
        }
    }
    Some(row)
}

/// 补好的一行写回索引。写不进的记一行：索引是派生的，下次列的时候再补。
fn put(index: Option<&SessionIndex>, row: &Row, was: Option<&Mark>) {
    if let Some(Err(error)) = index.map(|index| index.put(row, was)) {
        tracing::warn!(target: "miyu::endpoint", session = row.id.as_str(), error = %error, "session index not updated");
    }
}

/// 打开账号 `account` 的会话列表的索引（施工 3-8 七补）：核心起来时开一次，一直开着。新建的记一行 `INFO`；读不了、坏了、
/// 版本不对，删掉换了一份空的，记一行 `WARN`，列会话时照日志补；删了重建也打不开的记一行 `WARN`，每次列会话都整份读。
pub(crate) fn open_index(root: &DataRoot, account: &AccountId) -> SessionIndex {
    let (index, opened) = SessionIndex::open(&root.index(account).join(FILE));
    match opened {
        Opened::Kept => {}
        Opened::Created => tracing::info!(target: "miyu::endpoint", "session index created"),
        Opened::Rebuilt(why) => {
            tracing::warn!(target: "miyu::endpoint", reason = %why, "session index rebuilt");
        }
        Opened::Unusable(error) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "session index unusable");
        }
    }
    index
}

/// 删掉的会话（挪进回收处的）从索引里拿掉那一行。拿不掉的记一行：列会话照放会话的目录走，那一行不碍事。
pub(crate) fn forget(index: &SessionIndex, id: &SessionId) {
    if let Err(error) = index.remove(id) {
        tracing::warn!(target: "miyu::endpoint", session = id.as_str(), error = %error, "session index row not removed");
    }
}

#[cfg(test)]
mod tests;

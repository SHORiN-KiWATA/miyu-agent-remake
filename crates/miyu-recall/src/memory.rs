//! 记下的记忆：记忆日志里的事件怎么写、怎么读，照它算出的底账（`docs/blueprint/memory.md`「对外的样子」的记忆日志、底账，
//! 施工 R-3 上）。
//!
//! - 事件用内核的外壳，种类是 `ext.memory.*`：内核不认识，原样留着（`03-事件模型.md` 第三节）；这里照自己的格读写 `body`。
//! - 只追加：改一条是新写一条 `saved` 指着旧的（`replaces`），作废是另写一条 `retired`，原文一直在日志里（mem0 弃了原地
//!   改写，`docs/reviews/2026-10-07-记忆知识库embedding调研.md` 第二节）。
//! - 底账 [`MemoryBook`] 只是照日志算：改一条不存在的、作废一条不存在的，照样记下这条事件，不影响别的；日志是真相，底账
//!   不拒。

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{CommandId, EventKind, Seq, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::raw::RawJson;
use miyu_kernel::time::Timestamp;

/// 记下一条（写了 `replaces` 的是改一条）。
const SAVED: &str = "ext.memory.saved";
/// 作废一条。
const RETIRED: &str = "ext.memory.retired";
/// 清空（施工 R-3 补）。
const CLEARED: &str = "ext.memory.cleared";

/// 出厂认识的四类（2026-10-07 项目主人定）：关于你、你要她怎样、经历、长期有效的事实。日志里别的类原样留着、照常列出；
/// 她经工具记的照这张名单查（R-3 中）。
pub const CLASSES: [&str; 4] = ["user", "feedback", "episode", "reference"];

/// 一条记忆的编号：它在记忆日志里的序号前面加 `m`，例如 `m42`。从序号算，不写进 `body`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoryId(Seq);

impl MemoryId {
    /// 记忆日志里第 `seq` 条记下的那一条。
    pub fn new(seq: Seq) -> MemoryId {
        MemoryId(seq)
    }

    /// 读 `m42` 这种写法；别的写法（没有 `m`、大写、0、不是数）是 `None`。
    pub fn parse(text: &str) -> Option<MemoryId> {
        let digits = text.strip_prefix('m')?;
        if !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok().and_then(Seq::new).map(MemoryId)
    }

    /// 它在记忆日志里的序号。
    pub fn seq(self) -> Seq {
        self.0
    }
}

impl fmt::Display for MemoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "m{}", self.0.get())
    }
}

impl Serialize for MemoryId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for MemoryId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        MemoryId::parse(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("not a memory id: {text}")))
    }
}

/// 一处出处：哪个会话的哪一轮。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// 会话编号。
    pub session: SessionId,
    /// 回合编号。
    pub turn: TurnId,
}

/// `ext.memory.saved` 的 `body`：记下一条，写了 `replaces` 的是改那一条。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Saved {
    /// 类：出厂四类之一（[`CLASSES`]），别的原样留着。
    pub class: String,
    /// 正文，一条不超过 120 字（工具查，R-3 中）。
    pub text: String,
    /// 出处，几处；人在界面上记的是空的。
    pub sources: Vec<Source>,
    /// 听众：出处那几轮会被谁看到，几个「谁」（`17-记忆.md` L6）。
    pub audience: Vec<By>,
    /// 改的是哪一条；没有的是新记的。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<MemoryId>,
    /// 说的是哪天的事，写法自由；没有的不写。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub about: Option<String>,
}

/// `ext.memory.retired` 的 `body`：作废一条。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retired {
    /// 作废哪一条。
    pub id: MemoryId,
    /// 为什么。
    pub why: String,
}

/// `ext.memory.cleared` 的 `body`：人清空了（施工 R-3 补，`memory.md` 第二条第 5 款）。那以前的、合条件的都清掉，原文还在
/// 日志里。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cleared {
    /// 只清从这个会话来的（出处全在它里面的）；没有的是整间。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionId>,
}

/// 记忆日志里的一条事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryEvent {
    /// 记下（或改）一条。
    Saved(Saved),
    /// 作废一条。
    Retired(Retired),
    /// 清空（施工 R-3 补）。
    Cleared(Cleared),
}

/// 写成记忆日志里的一条事件：第 `seq` 条，时刻 `at`，`by` 写它的那一方。
///
/// # Errors
///
/// `body` 写不成 JSON（不会发生：格都是字符串、数和列表），如实交回。
pub fn to_event(seq: Seq, at: Timestamp, by: By, event: &MemoryEvent) -> Result<Event, String> {
    let (kind, json) = match event {
        MemoryEvent::Saved(saved) => (SAVED, serde_json::to_string(saved)),
        MemoryEvent::Retired(retired) => (RETIRED, serde_json::to_string(retired)),
        MemoryEvent::Cleared(cleared) => (CLEARED, serde_json::to_string(cleared)),
    };
    let json = json.map_err(|error| error.to_string())?;
    let body: RawJson = serde_json::from_str(&json).map_err(|error| error.to_string())?;
    let kind = EventKind::parse(kind).map_err(|error| error.to_string())?;
    Ok(Event {
        seq,
        at,
        turn: None,
        by,
        cause: None,
        body: Body::Unknown { kind, body },
    })
}

/// 读一条事件：不是记忆的种类交回 `None`；是记忆的、`body` 读不懂的交回为什么。
pub fn from_event(event: &Event) -> Option<Result<MemoryEvent, String>> {
    let Body::Unknown { kind, body } = &event.body else {
        return None;
    };
    let read = match kind.as_str() {
        SAVED => serde_json::from_str(body.get()).map(MemoryEvent::Saved),
        RETIRED => serde_json::from_str(body.get()).map(MemoryEvent::Retired),
        CLEARED => serde_json::from_str(body.get()).map(MemoryEvent::Cleared),
        _ => return None,
    };
    Some(read.map_err(|error| format!("body of {kind} not readable: {error}")))
}

/// 底账里的一条：现在的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 编号。
    pub id: MemoryId,
    /// 类。
    pub class: String,
    /// 正文。
    pub text: String,
    /// 出处。
    pub sources: Vec<Source>,
    /// 听众。
    pub audience: Vec<By>,
    /// 说的是哪天的事。
    pub about: Option<String>,
    /// 记下的时刻。
    pub at: Timestamp,
    /// 谁记的。
    pub by: By,
    /// 被哪一条改掉了。
    pub replaced_by: Option<MemoryId>,
    /// 作废了的为什么。
    pub retired: Option<String>,
    /// 清掉了（施工 R-3 补）：哪里都不出来，作废的一起也不出来。
    pub cleared: bool,
}

impl Entry {
    /// 现在还算数：没被改掉、没作废、没清掉。出处活不活另看（回合库的墓碑，`memory.md` 第二条第 4 款）。
    pub fn current(&self) -> bool {
        self.replaced_by.is_none() && self.retired.is_none() && !self.cleared
    }
}

/// 照记忆日志算出的底账。
#[derive(Debug, Default)]
pub struct MemoryBook {
    entries: BTreeMap<MemoryId, Entry>,
    /// 带命令编号（`cause`）的每一条做成了什么：那一条的编号、清掉几条（施工 R-3 补，04 第六节第 1 条：同一个编号只生效一次）。
    done: BTreeMap<CommandId, (MemoryId, usize)>,
}

impl MemoryBook {
    /// 看一条事件，交回清掉了几条（清空的；不算改掉的旧版本，别的是 0）。不是记忆的跳过。
    ///
    /// # Errors
    ///
    /// 是记忆的、`body` 读不懂：交回为什么，调的一方记日志，底账照旧。
    pub fn see(&mut self, event: &Event) -> Result<usize, String> {
        let Some(cleared) = self.apply(event)? else {
            return Ok(0);
        };
        if let Some(cause) = &event.cause {
            self.done
                .insert(cause.clone(), (MemoryId::new(event.seq), cleared));
        }
        Ok(cleared)
    }

    /// 命令编号 `cause` 做过的：那一条的编号、清掉几条；没做过的是 `None`。
    pub fn done(&self, cause: &CommandId) -> Option<(MemoryId, usize)> {
        self.done.get(cause).copied()
    }

    /// 算一条，交回清掉了几条（不是清空的是 0）；不是记忆的交回 `None`。
    fn apply(&mut self, event: &Event) -> Result<Option<usize>, String> {
        match from_event(event) {
            None => Ok(None),
            Some(Err(why)) => Err(why),
            Some(Ok(MemoryEvent::Saved(saved))) => {
                let id = MemoryId::new(event.seq);
                if let Some(old) = saved.replaces.and_then(|old| self.entries.get_mut(&old)) {
                    old.replaced_by = Some(id);
                }
                self.entries.insert(
                    id,
                    Entry {
                        id,
                        class: saved.class,
                        text: saved.text,
                        sources: saved.sources,
                        audience: saved.audience,
                        about: saved.about,
                        at: event.at,
                        by: event.by.clone(),
                        replaced_by: None,
                        retired: None,
                        cleared: false,
                    },
                );
                Ok(Some(0))
            }
            Some(Ok(MemoryEvent::Retired(retired))) => {
                if let Some(entry) = self.entries.get_mut(&retired.id) {
                    entry.retired = Some(retired.why);
                }
                Ok(Some(0))
            }
            Some(Ok(MemoryEvent::Cleared(cleared))) => {
                // 交回的条数不算改掉的旧版本：它们本来就看不见，人数的是看得见的那些（作废的带 `forgotten` 看得见）。
                let mut shown = 0;
                for id in self.clears(&cleared) {
                    if let Some(entry) = self.entries.get_mut(&id) {
                        entry.cleared = true;
                        shown += usize::from(entry.replaced_by.is_none());
                    }
                }
                Ok(Some(shown))
            }
        }
    }

    /// 现在追加 `cleared` 会清掉哪几条，照编号：还没清掉的里，整间的是全部；带会话的是出处全在这个会话里的（照 17 第六节
    /// 撤回的规矩：还有别的出处的不动），人记的（没有出处）不动。记忆库照它删字。
    pub fn clears(&self, cleared: &Cleared) -> Vec<MemoryId> {
        self.entries
            .values()
            .filter(|entry| !entry.cleared)
            .filter(|entry| match &cleared.session {
                None => true,
                Some(session) => {
                    !entry.sources.is_empty()
                        && entry
                            .sources
                            .iter()
                            .all(|source| &source.session == session)
                }
            })
            .map(|entry| entry.id)
            .collect()
    }

    /// 编号 `id` 的那一条；没有的是 `None`。
    pub fn get(&self, id: MemoryId) -> Option<&Entry> {
        self.entries.get(&id)
    }

    /// 全部，照编号：改掉的、作废的、清掉的也在。
    pub fn all(&self) -> impl DoubleEndedIterator<Item = &Entry> {
        self.entries.values()
    }
}

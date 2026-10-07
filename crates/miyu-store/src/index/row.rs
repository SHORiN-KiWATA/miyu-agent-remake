//! 索引里的一行（施工 3-8 七补，`docs/blueprint/store/index.md`「一行记什么」）：一个会话照日志算出来的几样，和它照到日志的
//! 哪里。会话写日志时一批批往上盖（[`super::SessionIndex::advance`]），列会话时照日志补（`miyu-endpoint` 的 `list.rs`）：两处
//! 照同一个 [`Row::see`] 算，结果才和从头读整份日志的一字不差。

use rusqlite::Row as SqlRow;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::time::Timestamp;

use super::IndexError;
use crate::log::Mark;

/// 一个会话在索引里的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// 会话编号。
    pub id: SessionId,
    /// 属主：`session.created` 的 `owner`。
    pub owner: AccountId,
    /// 父会话：子会话才有。
    pub parent: Option<SessionId>,
    /// 一次性的：`miyu ask` 开的。
    pub oneshot: bool,
    /// 标题：空的是没有。
    pub title: String,
    /// 置顶。
    pub pinned: bool,
    /// 工作目录：日志里最后一条带 `cwd` 的（[`cwd`]），很早以前的日志一条都没有。
    pub cwd: Option<String>,
    /// 造的时刻：`session.created` 的 `at`。
    pub created: Timestamp,
    /// 最近一次动静：照到的最后一条事件的 `at`。
    pub last_active: Timestamp,
    /// 照到日志的哪里。
    pub mark: Mark,
    /// 第一句话的第一行（施工 9-5）：第一条 `message.user` 的文字，去掉前后空白，最多 50 个字（`PREVIEW_CHARS`）；还没人说过话、
    /// 第一句没有字的是空的。没有标题的会话，头照它显示。
    pub preview: String,
}

/// `preview` 最多几个字（按 Unicode 的字数）。
const PREVIEW_CHARS: usize = 50;

impl Row {
    /// 照日志的第一条起一行：`session.created` 的属主、父会话、是不是一次性的、工作目录，时刻既是造的时刻也是最近一次
    /// 动静。照到哪里先记成第一段的开头，调的一方读完再换。第一条不是 `session.created` 的没有。
    pub fn new(id: SessionId, first: &Event) -> Option<Row> {
        let Body::SessionCreated(created) = &first.body else {
            return None;
        };
        Some(Row {
            id,
            owner: created.owner.clone(),
            parent: created.parent.clone(),
            oneshot: created.oneshot,
            title: String::new(),
            pinned: false,
            cwd: created.cwd.clone(),
            created: first.at,
            last_active: first.at,
            mark: Mark {
                segment: 1,
                bytes: 0,
                next: first.seq,
            },
            preview: String::new(),
        })
    }

    /// 照先后看一条（`protocol.md` 的 `session.list` 第 3、4 条）：它的时刻是最近一次动静；带 `cwd` 的盖上工作目录；
    /// `session.meta_changed` 写了 `title` 的换成它（空的是去掉），写了 `pinned` 的换成它。撤掉的回合里改的也算：改名不是
    /// 对话的一部分。
    pub fn see(&mut self, event: &Event) {
        self.last_active = event.at;
        if let Some(cwd) = cwd(event) {
            self.cwd = Some(cwd.to_string());
        }
        if let Body::MessageUser(message) = &event.body
            && self.preview.is_empty()
        {
            self.preview = preview(&message.blocks);
        }
        if let Body::MetaChanged(changed) = &event.body {
            if let Some(title) = &changed.title {
                self.title.clone_from(title);
            }
            self.pinned = changed.pinned.unwrap_or(self.pinned);
        }
    }

    /// 从表里读出来的一行，列的先后照 [`super::COLUMNS`]。
    pub(super) fn from_sql(row: &SqlRow<'_>) -> Result<Row, IndexError> {
        let id: String = row.get(0)?;
        let bad = |what: &str| IndexError::Bad(format!("row {id}: {what}"));
        let owner: String = row.get(1)?;
        let parent: Option<String> = row.get(2)?;
        let parent = match parent {
            Some(parent) => Some(SessionId::parse(&parent).map_err(|_| bad("parent"))?),
            None => None,
        };
        let millis = |at: i64| Timestamp::from_unix_millis(at).ok_or_else(|| bad("time"));
        let number = |n: i64| u64::try_from(n).map_err(|_| bad("mark"));
        let next: i64 = row.get(11)?;
        Ok(Row {
            id: SessionId::parse(&id).map_err(|_| bad("id"))?,
            owner: AccountId::parse(&owner).map_err(|_| bad("owner"))?,
            parent,
            oneshot: row.get(3)?,
            title: row.get(4)?,
            pinned: row.get(5)?,
            cwd: row.get(6)?,
            created: millis(row.get(7)?)?,
            last_active: millis(row.get(8)?)?,
            mark: Mark {
                segment: number(row.get(9)?)?,
                bytes: number(row.get(10)?)?,
                next: miyu_kernel::id::Seq::new(number(next)?).ok_or_else(|| bad("mark"))?,
            },
            preview: row.get(12)?,
        })
    }

    /// 写进表里的几格，先后照 [`super::COLUMNS`]。数字超过 SQLite 的整数（`i64`）的报错：一段不会有那么长。
    pub(super) fn to_sql(&self) -> Result<[rusqlite::types::Value; 13], IndexError> {
        use rusqlite::types::Value;
        let number = |n: u64| {
            i64::try_from(n)
                .map(Value::Integer)
                .map_err(|_| IndexError::Bad(format!("row {}: mark too large", self.id.as_str())))
        };
        let text = |s: &str| Value::Text(s.to_string());
        let optional = |s: Option<&str>| s.map_or(Value::Null, text);
        Ok([
            text(self.id.as_str()),
            text(self.owner.as_str()),
            optional(self.parent.as_ref().map(SessionId::as_str)),
            Value::Integer(i64::from(self.oneshot)),
            text(&self.title),
            Value::Integer(i64::from(self.pinned)),
            optional(self.cwd.as_deref()),
            Value::Integer(self.created.unix_millis()),
            Value::Integer(self.last_active.unix_millis()),
            number(self.mark.segment)?,
            number(self.mark.bytes)?,
            number(self.mark.next.get())?,
            text(&self.preview),
        ])
    }
}

/// 这一条记下的工作目录（`protocol.md`「会话表」第 5 条）：带 `cwd` 的 `turn.started`、`session.created`。会话表载入时、列会话时
/// 都照日志里最后一条带它的算，同一个认法。
pub fn cwd(event: &Event) -> Option<&str> {
    match &event.body {
        Body::TurnStarted(started) => started.cwd.as_deref(),
        Body::SessionCreated(created) => created.cwd.as_deref(),
        _ => None,
    }
}

/// 一句话的预览（施工 9-5）：头一块文字的第一行，去掉前后空白，最多 [`PREVIEW_CHARS`] 个字。没有文字的是空的。
fn preview(blocks: &[Block]) -> String {
    blocks
        .iter()
        .find_map(|block| match block {
            Block::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .and_then(|text| text.lines().map(str::trim).find(|line| !line.is_empty()))
        .map(|line| line.chars().take(PREVIEW_CHARS).collect())
        .unwrap_or_default()
}

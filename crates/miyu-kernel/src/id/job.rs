//! 任务编号：`j` 加一段或几段从 1 起的整数，段之间用 `.` 连（施工 7-1，蓝图 `kernel/ids.md`「任务编号」、第 25、26 条）。
//!
//! 一个会话里后台命令和子代理共用一串，数的是最后一段；前面的几段是这个会话在父会话里的编号，主会话没有：主会话派的是
//! `j2`，`j2` 派的是 `j2.1`，`j2.1` 放到后台的命令是 `j2.1.1`，一棵树上不重名（施工 7-1 补，`agents.md`「对外的样子」）。
//! 短，模型写得对；编号不回收，撤掉的回合里用过的也不再用，由账本查（`kernel/history.md`）。

use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::decimal;
use crate::format_error::FormatError;

/// 任务编号，写成 `j1`、`j12`、`j2.1`。JSON 里是字符串；排序一段一段照数比，`j2` 在 `j10` 前面，`j2` 在 `j2.1` 前面。
///
/// 里面是每一段的数，至少一段，每一段从 1 起：只有 [`JobId::new`]、[`JobId::under`]、[`JobId::parse`] 造得出来，都守着这两条。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobId(Vec<u64>);

impl JobId {
    /// 只有一段的任务编号 `j<n>`：主会话派的。从 1 数起，给 0 返回 `None`。
    pub fn new(n: u64) -> Option<JobId> {
        (n >= 1).then(|| JobId(vec![n]))
    }

    /// 这个编号后面接一段 `n`：编号是它的子会话派的第 `n` 个任务，`j2` 的 `under(1)` 是 `j2.1`（施工 7-1 补）。从 1 数起，
    /// 给 0 返回 `None`。
    pub fn under(&self, n: u64) -> Option<JobId> {
        (n >= 1).then(|| {
            let mut parts = self.0.clone();
            parts.push(n);
            JobId(parts)
        })
    }

    /// 最后一段的数：派它的会话里的第几个。执行器接着领号照它数（`kernel/history.md` 的 `last_job`）。
    ///
    /// # Panics
    ///
    /// 实际不会：造得出来的编号至少一段（类型的规矩）。
    pub fn last(&self) -> u64 {
        self.0
            .last()
            .copied()
            .unwrap_or_else(|| unreachable!("任务编号至少一段"))
    }

    /// 读 `j12`、`j2.1` 这样的写法。
    ///
    /// # Errors
    ///
    /// 只认内核自己写出去的样子：不以 `j` 开头的；后面照 `.` 切开，有一段不是从 1 起的十进制数的（例如 `j0`、`j01`、
    /// `j+1`、`j1.0`、`j1.`、`j.1`、超出 `u64` 的），返回 [`FormatError`]。
    pub fn parse(text: &str) -> Result<JobId, FormatError> {
        let bad = |why| FormatError::new("job id", text, why);
        let numbers = text
            .strip_prefix('j')
            .ok_or_else(|| bad("must start with j"))?;
        // `decimal` 不认以 0 开头的，`0` 也在里面：每一段都从 1 起。空的段（`j`、`j1.`、`j.1`、`j1..2`）也不认。
        numbers
            .split('.')
            .map(decimal)
            .collect::<Option<Vec<u64>>>()
            .map(JobId)
            .ok_or_else(|| bad("needs decimal numbers from 1 after j, joined by dots"))
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut separator = "j";
        for part in &self.0 {
            write!(f, "{separator}{part}")?;
            separator = ".";
        }
        Ok(())
    }
}

impl Serialize for JobId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for JobId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        JobId::parse(&String::deserialize(d)?).map_err(D::Error::custom)
    }
}

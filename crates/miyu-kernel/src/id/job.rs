//! 任务编号：`j` 加一个从 1 起的整数（施工 7-1，蓝图 `kernel/ids.md`「任务编号」、第 25、26 条）。
//!
//! 一个会话里后台命令和子代理共用一串，子会话自己派的另从 `j1` 数（`agents.md`）。短，模型写得对；编号不回收，撤掉的
//! 回合里用过的也不再用，由账本查（`kernel/history.md`）。

use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::decimal;
use crate::format_error::FormatError;

/// 任务编号，写成 `j1`、`j12`。JSON 里是字符串；排序照那个数，`j2` 在 `j10` 前面。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobId(u64);

impl JobId {
    /// 由数字得到任务编号。从 1 数起，给 0 返回 `None`。
    pub fn new(n: u64) -> Option<JobId> {
        (n >= 1).then_some(JobId(n))
    }

    /// 编号里的那个数。
    pub fn get(self) -> u64 {
        self.0
    }

    /// 读 `j12` 这样的写法。
    ///
    /// # Errors
    ///
    /// 只认内核自己写出去的样子：不以 `j` 开头的，后面不是从 1 起的十进制数的（例如 `j0`、`j01`、`j+1`、超出
    /// `u64` 的），返回 [`FormatError`]。
    pub fn parse(text: &str) -> Result<JobId, FormatError> {
        let bad = |why| FormatError::new("job id", text, why);
        let number = text
            .strip_prefix('j')
            .ok_or_else(|| bad("must start with j"))?;
        decimal(number)
            .and_then(JobId::new)
            .ok_or_else(|| bad("needs a decimal number from 1 after j"))
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "j{}", self.0)
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

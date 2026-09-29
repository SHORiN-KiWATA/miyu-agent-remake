//! 调用编号：`call_<助手消息的序号>_<这条消息里的第几个调用>`（`docs/designs/03-事件模型.md` 第二节，蓝图
//! `kernel/ids.md` 第 9 到 12 条）。

use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Seq, decimal};
use crate::format_error::FormatError;

/// 调用编号：`call_<助手消息的序号>_<这条消息里的第几个调用>`，从 1 数起，由内核分配。
/// 带着序号，一个会话里不会重复；供应商自己的编号放在驱动私有数据里。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallId {
    message: Seq,
    index: u32,
}

impl CallId {
    /// `index` 从 1 数起，0 不是调用编号。
    pub fn new(message: Seq, index: u32) -> Option<CallId> {
        (index >= 1).then_some(CallId { message, index })
    }

    /// 发起这个调用的助手消息的序号。
    pub fn message(self) -> Seq {
        self.message
    }

    /// 这是那条助手消息里的第几个调用，从 1 数起。
    pub fn index(self) -> u32 {
        self.index
    }

    /// 读 `call_44_1` 这样的写法。
    ///
    /// # Errors
    ///
    /// 只认内核自己写出去的样子。前缀不对、少了一段、数字不是从 1 开始的十进制写法
    /// （例如 `0`、`01`、`+1`），都返回 [`FormatError`]。
    pub fn parse(text: &str) -> Result<CallId, FormatError> {
        let bad = |why| FormatError::new("call id", text, why);
        let rest = text
            .strip_prefix("call_")
            .ok_or_else(|| bad("must start with call_"))?;
        let (message, index) = rest
            .split_once('_')
            .ok_or_else(|| bad("write it as call_<seq>_<index>"))?;
        let message = decimal(message)
            .and_then(Seq::new)
            .ok_or_else(|| bad("seq must be a decimal number from 1"))?;
        decimal(index)
            .and_then(|n| u32::try_from(n).ok())
            .and_then(|n| CallId::new(message, n))
            .ok_or_else(|| bad("index must be a decimal number from 1"))
    }
}

impl fmt::Display for CallId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "call_{}_{}", self.message, self.index)
    }
}

impl Serialize for CallId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for CallId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        CallId::parse(&String::deserialize(d)?).map_err(D::Error::custom)
    }
}

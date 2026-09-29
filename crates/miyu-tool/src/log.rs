//! 这个会话日志的只读入口（`docs/blueprint/tools/interface.md`，施工 6-4）：`history` 翻会话自己的记录用。
//!
//! 工具不知道日志存在哪、怎么存：执行器照会话的目录造一个 [`ReadLog`]，包成 [`Log`] 交给这次调用。

use std::fmt;
use std::sync::Arc;

use miyu_kernel::event::Event;

/// 一段一段读这个会话的日志，只读。
pub trait ReadLog: Send + Sync {
    /// 照段的先后，读一段交一段给 `each`，它交回 `false` 就不读下去。最后一段末尾没写完的半行跳过，一个字节
    /// 都不写：会话正在往里写。
    ///
    /// # Errors
    ///
    /// 读不了、日志坏了：原因，给她看。
    fn read(&self, each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String>;
}

/// 交给一次调用的日志入口；克隆出来的是同一个。
#[derive(Clone)]
pub struct Log(Arc<dyn ReadLog>);

impl Log {
    /// 包一个 [`ReadLog`]。
    pub fn new(read: impl ReadLog + 'static) -> Log {
        Log(Arc::new(read))
    }

    /// 同 [`ReadLog::read`]。
    ///
    /// # Errors
    ///
    /// 同 [`ReadLog::read`]。
    pub fn read(&self, mut each: impl FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        self.0.read(&mut each)
    }
}

impl fmt::Debug for Log {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Log")
    }
}

/// 两个入口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for Log {
    fn eq(&self, other: &Log) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Log {}

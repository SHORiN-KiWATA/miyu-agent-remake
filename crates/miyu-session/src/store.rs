//! 写盘的端口（施工 3-7 中「我定的」）：actor 追加的事件往哪里写。平时是会话日志；测试里换成写不进去
//! 的，查停下的那条路。不对外。撤销、恢复以后重算她看过的，从这里重读一遍日志（施工 4-7 上）。
//!
//! 交给 `history` 的日志只读入口也在这里（施工 6-4）：照会话的目录一段一段读。

use std::io;
use std::path::PathBuf;

use miyu_kernel::event::Event;
use miyu_store::log::{SessionLog, read_events, read_segments};
use miyu_tool::ReadLog;

/// 一次写一批，返回时这一批都落了盘。在阻塞线程里用，所以要能挪到别的线程上。
pub(crate) trait Store: Send + 'static {
    /// 追加一批。
    fn append(&mut self, events: &[Event]) -> io::Result<()>;

    /// 只读地读回整份日志。
    fn events(&self) -> Result<Vec<Event>, String>;
}

impl Store for SessionLog {
    fn append(&mut self, events: &[Event]) -> io::Result<()> {
        SessionLog::append(self, events)
    }

    fn events(&self) -> Result<Vec<Event>, String> {
        read_events(self.dir()).map_err(|error| error.to_string())
    }
}

/// 这个会话日志的只读入口（施工 6-4）：会话的目录。
pub(crate) struct LogDir(pub(crate) PathBuf);

impl ReadLog for LogDir {
    fn read(&self, each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        read_segments(&self.0, each).map_err(|error| error.to_string())
    }
}

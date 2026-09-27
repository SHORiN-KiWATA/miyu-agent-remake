//! 写盘的端口（施工 3-7 中「我定的」）：actor 追加的事件往哪里写。平时是会话日志；测试里换成写不进去
//! 的，查停下的那条路。不对外。撤销、恢复以后重算她看过的，从这里重读一遍日志（施工 4-7 上）。

use std::io;

use miyu_kernel::event::Event;
use miyu_store::log::{SessionLog, read_events};

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

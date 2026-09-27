//! 写盘的端口（施工 3-7 中「我定的」）：actor 追加的事件往哪里写。平时是会话日志；测试里换成写不进去
//! 的，查停下的那条路。不对外。

use std::io;

use miyu_kernel::event::Event;
use miyu_store::log::SessionLog;

/// 一次写一批，返回时这一批都落了盘。在阻塞线程里用，所以要能挪到别的线程上。
pub(crate) trait Store: Send + 'static {
    /// 追加一批。
    fn append(&mut self, events: &[Event]) -> io::Result<()>;
}

impl Store for SessionLog {
    fn append(&mut self, events: &[Event]) -> io::Result<()> {
        SessionLog::append(self, events)
    }
}

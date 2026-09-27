//! 会话的时钟和会话编号（`02-内核.md` 第七节「会话 actor 怎么跑」）。内核是纯逻辑，不看钟、不造
//! 随机数：送进去的时刻、新会话的编号都由这里给。

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use uuid::ContextV7;

use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;

/// 时刻的范围里最后的那一毫秒：公元 9999 年的最后一刻（`03-事件模型.md` 第二节的时刻写法）。
const LAST: i64 = 253_402_300_799_999;

/// 一个会话的时钟：系统时间，到毫秒。系统时间往回拨了，照上一次的：一个会话里的时刻不往回走。
#[derive(Debug, Default)]
pub(crate) struct Clock {
    last: i64,
}

impl Clock {
    /// 从 `at` 这一刻起不往回走：载入时照日志里最后一条的时刻。
    pub(crate) fn since(at: Timestamp) -> Clock {
        Clock {
            last: at.unix_millis(),
        }
    }

    /// 现在。
    pub(crate) fn now(&mut self) -> Timestamp {
        self.at(system_millis())
    }

    /// 系统时间是 `millis` 的时候，这一刻记成什么。
    fn at(&mut self, millis: i64) -> Timestamp {
        self.last = self.last.max(millis).min(LAST);
        Timestamp::from_unix_millis(self.last).unwrap_or_else(|| {
            unreachable!("0 到 LAST 之间的毫秒数都在时刻的范围里：{}", self.last)
        })
    }
}

/// 系统时间，Unix 纪元以来的毫秒。系统时间在 1970 年以前的，当 0。
fn system_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
        })
}

/// 造会话编号的计数器，一个核心进程共用一份（施工 3-9 补）：一个数据根只有一个核心，数据根里的会话编号
/// 就都照造的先后。
static ORDER: Mutex<ContextV7> = Mutex::new(ContextV7::new());

/// 一个新的会话编号：UUIDv7（`03-事件模型.md` 第二节），前 48 位是 `at` 这一刻的毫秒，照时间排得开；
/// 毫秒以下的几位是计数器，同一毫秒里造的照先后（RFC 9562 第 6.2 节，施工 3-9 补）；其余是系统给的随机数。
/// 系统时间往回拨了，照上一次的毫秒：后造的编号不会排到前面去。
pub fn new_id(at: Timestamp) -> SessionId {
    id_with(at, &ORDER)
}

/// 照 `order` 这份计数器造编号。测试各带一份新的，不和别的测试同时造的编号串在一起。
fn id_with(at: Timestamp, order: &Mutex<ContextV7>) -> SessionId {
    let millis = u64::try_from(at.unix_millis()).unwrap_or(0);
    let when = uuid::Timestamp::from_unix(
        order,
        millis / 1000,
        u32::try_from(millis % 1000).unwrap_or(0) * 1_000_000,
    );
    let text = uuid::Uuid::new_v7(when).hyphenated().to_string();
    SessionId::parse(&text)
        .unwrap_or_else(|e| unreachable!("UUID 的写法就是会话编号的写法：{text}：{e}"))
}

#[cfg(test)]
mod tests;

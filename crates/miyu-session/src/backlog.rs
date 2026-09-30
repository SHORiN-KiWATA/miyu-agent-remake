//! 订阅时要补发的那一截（施工 3-8 六补，`docs/blueprint/session/actor.md` 第 6 条，`04-核心协议.md` 第七节）：日志里序号
//! 大于 `after`、到订阅那一刻落了盘的最后一条为止的事件。
//!
//! 补到哪一条（`upto`）和订阅在 actor 的同一步里拿：那一步之前落了盘的都推过了，之后的都还没推，所以补的和之后推的
//! 接得上，不重不漏。读日志不在 actor 里：交回一个 [`Backlog`]，拿着它的一方（协议端点）另起阻塞线程读，会话照常跑。
//! `upto` 以前的都落了盘，日志只往后追加，什么时候读都一样；读的时候多出来的不要，它们从订阅推过去。

use miyu_kernel::event::Event;
use miyu_tool::Log;

use crate::blocking::blocking;

/// 补发的那一截：序号在 `after` 之后、`upto` 为止（含）的事件，还没读。
#[derive(Debug)]
pub struct Backlog {
    log: Log,
    after: u64,
    upto: u64,
}

impl Backlog {
    pub(crate) fn new(log: Log, after: u64, upto: u64) -> Backlog {
        Backlog { log, after, upto }
    }

    /// 补到哪一条：订阅那一刻落了盘的最后一条，也就是订阅推过来的第一条的前一条。它比 `after` 小的，什么都不补。
    pub fn upto(&self) -> u64 {
        self.upto
    }

    /// 在阻塞线程里只读地读出这一截，照先后。一次读完再交回（施工 3-8 六补定：载入会话本来就整份读进内存，不分批）。
    /// 没有要补的（`after` 不比 `upto` 小），不读日志。
    ///
    /// # Errors
    ///
    /// 日志读不了、坏了：原因。会话在这时被删了（目录挪走了）也在这里。
    pub async fn read(self) -> Result<Vec<Event>, String> {
        let Backlog { log, after, upto } = self;
        if after >= upto {
            return Ok(Vec::new());
        }
        blocking(move || {
            let mut kept = Vec::new();
            log.read(|segment| {
                kept.extend(segment.into_iter().filter(|event| {
                    let seq = event.seq.get();
                    seq > after && seq <= upto
                }));
                true
            })?;
            Ok(kept)
        })
        .await
    }
}

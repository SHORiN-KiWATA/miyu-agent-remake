//! 闲够了的会话退下（`docs/designs/07-存储.md` 第七节「会话按需载入」第 3 条，施工 V-2 再补）：会话表第一次放进会话时起一个
//! 任务，隔一阵拿着表的锁问一遍。没有别的把手、没人订阅、没有在跑的回合的，问它的 actor 闲够了没有（`Handle::retire`，它照
//! 自己的账答）；退下了的、已经停了的从表里拿掉，下次用到再载入。actor 退出时把放下的内存还给系统（`miyu-heap`）。
//!
//! 拿着锁问：这一刻谁也拿不到新的把手，退下和再载入不会同时有两个写者。下一次什么时候问：差得最少的那一个还差多久；都没得
//! 问的，过 `idle` 再问（这一刻还有事的，最早也要再闲 `idle` 才退得下）。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::Mutex;

use miyu_session::Retire;

use super::{Open, Sessions};
use crate::Core;

/// 闲多久退下：三分钟（`docs/blueprint/perf.md` 量尺的大会话那一项照它等）。
pub(crate) const IDLE: Duration = Duration::from_secs(180);

/// 两次问之间至少隔多久：差几毫秒的不连着问。
const LEAST: Duration = Duration::from_millis(10);

impl Default for Sessions {
    fn default() -> Sessions {
        Sessions::new(IDLE)
    }
}

impl Sessions {
    /// 空的会话表，闲 `idle` 退下。
    fn new(idle: Duration) -> Sessions {
        Sessions {
            open: Mutex::new(Open::default()),
            idle,
            sweeping: AtomicBool::new(false),
        }
    }

    /// 起看空闲的任务（起过的不再起）：只拿着核心的弱引用，核心放下了就停。
    pub(super) fn sweep(&self, core: &Arc<Core>) {
        if self.sweeping.swap(true, Ordering::AcqRel) {
            return;
        }
        let (core, idle) = (Arc::downgrade(core), self.idle);
        tokio::spawn(async move {
            let mut wait = idle;
            loop {
                tokio::time::sleep(wait).await;
                let Some(core) = core.upgrade() else {
                    return;
                };
                wait = core.sessions.retire_idle().await.max(LEAST);
            }
        });
    }

    /// 问一遍：退下了的、停了的拿掉。交回下一次隔多久再问。
    async fn retire_idle(&self) -> Duration {
        let mut open = self.open.lock().await;
        let mut next = self.idle;
        let mut gone = Vec::new();
        for (id, running) in &open.running {
            let handle = &running.handle;
            if handle.busy() || handle.watched() || !handle.alone() {
                continue;
            }
            match handle.retire(self.idle).await {
                Ok(Retire::Retired) | Err(_) => gone.push(id.clone()),
                Ok(Retire::Later(left)) => next = next.min(left),
                Ok(Retire::Kept) => {}
            }
        }
        for id in gone {
            open.running.remove(&id);
        }
        next
    }
}

impl Core {
    /// 同一份家底，会话闲 `idle` 退下（施工 V-2 再补）：测试里设短的，不用真等三分钟。
    #[must_use]
    pub fn with_session_idle(mut self, idle: Duration) -> Core {
        self.sessions = Sessions::new(idle);
        self
    }

    /// 这时载入着几个会话（施工 V-2 再补）：测试看闲够了的退下了没有。
    pub async fn loaded(&self) -> usize {
        self.sessions.open.lock().await.running.len()
    }
}

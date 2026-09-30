//! 读按键的线程（蓝图 `tui.md`「她的回答：Markdown」第 10 条）：一直读终端的事件送进主循环；让出终端给编辑器时能
//! 停下，不跟编辑器抢键。停着时每一小会儿看一眼要不要接着读。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event};

/// 一次最多等多久有没有按键：停下的时候最多等这么久。
const POLL: Duration = Duration::from_millis(50);

/// 读按键的线程的开关。
#[derive(Clone, Default)]
pub struct Reader {
    /// 要它停下。
    paused: Arc<AtomicBool>,
    /// 它停下了（不在读）。
    idle: Arc<AtomicBool>,
}

impl Reader {
    /// 起一个线程读终端的事件，交给 `send`；`send` 交回假（主循环不收了）或者终端读不了时收工。
    pub fn spawn(send: impl Fn(Event) -> bool + Send + 'static) -> Self {
        let reader = Self::default();
        let (paused, idle) = (reader.paused.clone(), reader.idle.clone());
        thread::spawn(move || {
            loop {
                if paused.load(Ordering::Acquire) {
                    idle.store(true, Ordering::Release);
                    thread::sleep(POLL);
                    continue;
                }
                idle.store(false, Ordering::Release);
                match event::poll(POLL) {
                    Ok(false) => {}
                    Ok(true) => {
                        let sent = event::read().map(&send);
                        if !matches!(sent, Ok(true)) {
                            return;
                        }
                    }
                    Err(_) => return,
                }
            }
        });
        reader
    }

    /// 停下：等它不在读了才交回（最多等一秒）。
    pub fn pause(&self) {
        self.paused.store(true, Ordering::Release);
        let until = Instant::now() + Duration::from_secs(1);
        while !self.idle.load(Ordering::Acquire) && Instant::now() < until {
            thread::sleep(Duration::from_millis(5));
        }
    }

    /// 接着读。
    pub fn resume(&self) {
        self.paused.store(false, Ordering::Release);
    }
}

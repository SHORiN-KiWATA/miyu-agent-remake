//! 测试用的后台进程（施工 7-3）：一直跑，直到测试让它结束，或者被整组杀掉。任务表、会话、核心的测试用它，不起真的进程，
//! 三个平台一样。

use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use crate::{Background, Exit, Process};

/// 被杀时先跑的检查：例如读一眼会话日志，看结束的记录是不是已经落了盘。
type Check = Box<dyn Fn() + Send + Sync>;

/// 一条假的后台命令：输出是给的几段，一交出去就读完；进程一直跑到 [`Held::end`] 或者 [`Process::kill`]。
pub struct Held {
    output: Vec<String>,
    exit: Mutex<Option<Exit>>,
    changed: Condvar,
    killed: AtomicUsize,
    on_kill: Mutex<Option<Check>>,
}

impl Held {
    /// 输出是 `output` 这几段，照先后。
    pub fn new(output: &[&str]) -> Arc<Held> {
        Arc::new(Held {
            output: output.iter().map(|piece| (*piece).to_string()).collect(),
            exit: Mutex::new(None),
            changed: Condvar::new(),
            killed: AtomicUsize::new(0),
            on_kill: Mutex::new(None),
        })
    }

    /// 交给任务端口的那一份：输出、进程都是它。
    pub fn background(self: &Arc<Held>) -> Background {
        Background {
            output: Box::new(self.output.clone().into_iter()),
            process: Box::new(Arc::clone(self)),
        }
    }

    /// 让它自己结束：退出码或者信号照 `exit`。已经结束了的不变。
    pub fn end(&self, exit: Exit) {
        let mut state = self.lock();
        if state.is_none() {
            *state = Some(exit);
            self.changed.notify_all();
        }
    }

    /// 在跑的时候被整组杀了几次。
    pub fn killed(&self) -> usize {
        self.killed.load(Ordering::Acquire)
    }

    /// 结束了没有。
    pub fn ended(&self) -> bool {
        self.lock().is_some()
    }

    /// 被杀的时候，先跑一遍 `check`。
    pub fn on_kill(&self, check: impl Fn() + Send + Sync + 'static) {
        *self.on_kill.lock().unwrap_or_else(PoisonError::into_inner) = Some(Box::new(check));
    }

    fn lock(&self) -> MutexGuard<'_, Option<Exit>> {
        self.exit.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Process for Arc<Held> {
    fn wait(&self) -> std::io::Result<Exit> {
        let mut state = self.lock();
        loop {
            if let Some(exit) = *state {
                return Ok(exit);
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// 在跑的：先跑检查，数一次，当它被 `SIGKILL` 杀掉了。已经结束了的什么都不做。
    fn kill(&self) {
        if self.ended() {
            return;
        }
        if let Some(check) = &*self.on_kill.lock().unwrap_or_else(PoisonError::into_inner) {
            check();
        }
        self.killed.fetch_add(1, Ordering::AcqRel);
        self.end(Exit::Signal(9));
    }
}

impl fmt::Debug for Held {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Held")
            .field("output", &self.output)
            .field("exit", &*self.lock())
            .field("killed", &self.killed())
            .finish_non_exhaustive()
    }
}

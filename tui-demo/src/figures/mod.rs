//! 正文里的图（蓝图 `tui.md`「图片、公式和 mermaid 图」）：记着做好的图，没有的交给后台线程做。
//!
//! 画正文时每一张图来问一声（[`Figures::look`]）：做好了交回几行，没好是占位，
//! 终端显示不了图的、出错的由画的那一层写源码。

mod cells;
mod file;
mod math;
mod mermaid;
pub mod terminal;
mod worker;

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use ratatui_image::sliced::SlicedProtocol;

use crate::config::FigureLook;
use crate::markdown::FigureKind;
use crate::theme;
pub use terminal::Graphics;
pub use worker::Done;
use worker::Job;

/// 做好的一张图：按终端的协议编好了，切成一行行。
pub struct Drawn {
    /// 编好的图。
    pub protocol: SlicedProtocol,
    /// 占几行。
    pub rows: u16,
    /// 点开看的大图（mermaid 才有）：文件的路径。
    pub zoom: Option<PathBuf>,
}

/// 一张图现在怎样。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// 终端显示不了图：写源码。
    Unsupported,
    /// 在做：写一行占位。
    Pending,
    /// 做好了：占 `rows` 行，按 `key` 取来画。
    Ready {
        /// 取图用的键。
        key: u64,
        /// 占几行。
        rows: u16,
    },
    /// 出错了：写源码。
    Failed,
}

enum Slot {
    Pending,
    Ready(Drawn),
    Failed,
}

/// 做好的图，和送活的口子。
pub struct Figures {
    slots: HashMap<u64, Slot>,
    /// 记进来的先后，记满了扔最早的。
    order: VecDeque<u64>,
    /// 后台线程；终端显示不了图时没有。
    jobs: Option<Sender<Job>>,
    keep: usize,
}

impl Figures {
    /// 终端能显示图的起后台线程；`zoom_dir` 是点开看的 mermaid 大图放在哪；
    /// `notify` 在做好一张时被调，交回假表示主循环没了。
    pub fn start(
        graphics: Option<Graphics>,
        look: &FigureLook,
        zoom_dir: Option<PathBuf>,
        notify: impl Fn(Done) -> bool + Send + 'static,
    ) -> Self {
        let jobs = graphics.map(|g| worker::spawn(g, look.clone(), zoom_dir, notify));
        Self::with_jobs(jobs, look.keep)
    }

    fn with_jobs(jobs: Option<Sender<Job>>, keep: usize) -> Self {
        Self {
            slots: HashMap::new(),
            order: VecDeque::new(),
            jobs,
            keep: keep.max(1),
        }
    }

    /// 这一张现在怎样；没做过的交给后台去做。`cols` 是最多几列宽。
    pub fn look(&mut self, kind: FigureKind, source: &str, cols: u16) -> Look {
        let Some(jobs) = &self.jobs else {
            return Look::Unsupported;
        };
        let key = key(kind, source, cols);
        match self.slots.get(&key) {
            Some(Slot::Pending) => return Look::Pending,
            Some(Slot::Ready(drawn)) => {
                return Look::Ready {
                    key,
                    rows: drawn.rows,
                };
            }
            Some(Slot::Failed) => return Look::Failed,
            None => {}
        }
        let job = Job {
            key,
            kind,
            source: source.to_string(),
            cols,
            math: theme::math_rgb(),
            diagram: theme::diagram_rgb(),
        };
        if jobs.send(job).is_err() {
            return Look::Failed;
        }
        self.remember(key, Slot::Pending);
        Look::Pending
    }

    /// 做好的一张，照键取。
    pub fn get(&self, key: u64) -> Option<&Drawn> {
        match self.slots.get(&key) {
            Some(Slot::Ready(drawn)) => Some(drawn),
            _ => None,
        }
    }

    /// 忘掉做好的图，下次画时重做、重新传给终端：挂起回来以后用（终端离开全屏时可能把传过的图丢了）。
    pub fn forget(&mut self) {
        self.slots.clear();
        self.order.clear();
    }

    /// 后台做完一张。已经被扔掉的（记满了）不再记。
    pub fn done(&mut self, done: Done) {
        if let Some(slot) = self.slots.get_mut(&done.key) {
            *slot = match done.result {
                Ok(drawn) => Slot::Ready(drawn),
                Err(_) => Slot::Failed,
            };
        }
    }

    fn remember(&mut self, key: u64, slot: Slot) {
        self.slots.insert(key, slot);
        self.order.push_back(key);
        while self.order.len() > self.keep {
            if let Some(oldest) = self.order.pop_front() {
                self.slots.remove(&oldest);
            }
        }
    }
}

/// 一张图的键：种类、源码、宽度、换过几次主题（颜色烤在图里）。
fn key(kind: FigureKind, source: &str, cols: u16) -> u64 {
    let mut hasher = DefaultHasher::new();
    (kind, source, cols, theme::generation()).hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests;

//! 正文里的图（蓝图 `tui.md`「图片、公式和 mermaid 图」）：记着做好的图，没有的交给后台线程做。
//!
//! 画正文时每一张图来问一声（[`Figures::look`]）：做好了交回几行，没好是占位，
//! 终端显示不了图的、出错的由画的那一层写源码。

mod cells;
mod file;
mod math;
mod mermaid;
mod svg;
pub mod terminal;
mod worker;

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::mpsc::Sender;

use ratatui_image::sliced::SlicedProtocol;

use crate::config::{FigureLook, Room};
use crate::markdown::{FigureKind, Size};
use crate::theme;
pub use terminal::Graphics;
use worker::Job;
pub use worker::{Done, Outcome};

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

/// 一张图记着的样子。
enum Slot {
    /// 交给后台了，还没做好：照它的活再做（编好的图被扔了以后）。
    Pending(Job),
    /// 做好了：占几行一直记着；编好的图 `drawn` 可能被扔（没露出来、记满了），再露出来时照 `job` 重做一次。
    Ready {
        rows: u16,
        /// 占几列（链接卡片照它把字排在图右边）。
        cols: u16,
        drawn: Option<Drawn>,
        job: Job,
        /// 扔了以后又交给后台重做，还没回来。
        redoing: bool,
        /// 最近一次露出来是第几帧。
        seen: u64,
    },
    Failed,
}

/// 做好的图，和送活的口子。
pub struct Figures {
    slots: HashMap<u64, Slot>,
    /// 每张图（种类、源码、写的宽高）最近做好的是哪一份：换尺寸时新的还没好，先拿它顶着。
    latest: HashMap<u64, u64>,
    /// 后台线程；终端显示不了图时没有。
    jobs: Option<Sender<Job>>,
    /// 编好的图最多记几张（露着的超过它就先多记着）。
    keep: usize,
    /// 图的样子（在做、做好占几行、做坏）变一次就加一：正文按条缓存排好的行，图占几行变了要全排一遍
    /// （`ui/row_cache`）。扔编好的图、重做它都不变：占几行没变。
    revision: u64,
    /// 画到第几帧：扔图时认哪些这一帧露着。
    frame: u64,
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
            latest: HashMap::new(),
            jobs,
            keep: keep.max(1),
            revision: 0,
            frame: 0,
        }
    }

    /// 这一张现在怎样；没做过的交给后台去做。`size` 是 `<img>` 写的宽高，`cols`、`rows` 是最多几列宽、几行高。
    /// 编好的图被扔了的照样是做好了：占几行不变，露出来时再重做（[`Figures::shown`]）。
    pub fn look(
        &mut self,
        kind: FigureKind,
        source: &str,
        size: Size,
        cols: u16,
        rows: u16,
    ) -> Look {
        let Some(jobs) = &self.jobs else {
            return Look::Unsupported;
        };
        let key = key(kind, source, size, (cols, rows));
        let same = same(kind, source, size);
        match self.slots.get(&key) {
            Some(Slot::Pending(_)) => return self.stale(same),
            Some(Slot::Ready { rows, .. }) => return Look::Ready { key, rows: *rows },
            Some(Slot::Failed) => return Look::Failed,
            None => {}
        }
        let job = Job {
            key,
            same,
            kind,
            source: source.to_string(),
            size,
            cols,
            rows,
            math: theme::math_rgb(),
            diagram: theme::diagram_rgb(),
        };
        if jobs.send(job.clone()).is_err() {
            return Look::Failed;
        }
        self.slots.insert(key, Slot::Pending(job));
        self.stale(same)
    }

    /// 这张图新的尺寸还在做：有做好过的先拿它顶着（不闪「正在画图」），没有的是在做。
    fn stale(&self, same: u64) -> Look {
        let ready = self
            .latest
            .get(&same)
            .and_then(|key| match self.slots.get(key) {
                Some(Slot::Ready { rows, .. }) => Some(Look::Ready {
                    key: *key,
                    rows: *rows,
                }),
                _ => None,
            });
        ready.unwrap_or(Look::Pending)
    }

    /// 终端能不能显示图：不能的 mermaid 不用问核心，直接写源码。
    pub fn shows(&self) -> bool {
        self.jobs.is_some()
    }

    /// 做好的一张占几列；没做好的是 `None`。
    pub fn cols(&self, key: u64) -> Option<u16> {
        match self.slots.get(&key) {
            Some(Slot::Ready { cols, .. }) => Some(*cols),
            _ => None,
        }
    }

    /// 做好的一张，照键取；编好的图被扔了的是 `None`。
    pub fn get(&self, key: u64) -> Option<&Drawn> {
        match self.slots.get(&key) {
            Some(Slot::Ready { drawn, .. }) => drawn.as_ref(),
            _ => None,
        }
    }

    /// 画一帧开始：先扔上一帧没露出来的编好的图（记满了才扔，扔最久没露出来的），再数这一帧。
    pub fn next_frame(&mut self) {
        self.trim();
        self.frame += 1;
    }

    /// 这一帧露出来的一张：记下露过；编好的图被扔了的交给后台重做一次（重做着的不重复交），这一帧先不画。
    pub fn shown(&mut self, key: u64) -> Option<&Drawn> {
        let frame = self.frame;
        let Some(Slot::Ready {
            drawn,
            job,
            redoing,
            seen,
            ..
        }) = self.slots.get_mut(&key)
        else {
            return None;
        };
        *seen = frame;
        if drawn.is_none() && !*redoing {
            *redoing = self
                .jobs
                .as_ref()
                .is_some_and(|jobs| jobs.send(job.clone()).is_ok());
        }
        drawn.as_ref()
    }

    /// 忘掉做好的图，下次画时重做、重新传给终端：挂起回来以后用（终端离开全屏时可能把传过的图丢了）。
    pub fn forget(&mut self) {
        self.slots.clear();
        self.latest.clear();
        self.revision += 1;
    }

    /// 图的样子变过几次：做好、做坏都算；扔编好的图、重做它不算。
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// 后台做完一张。已经忘掉的（挂起回来以后）不再记。
    pub fn done(&mut self, done: Done) {
        let frame = self.frame;
        let Some(slot) = self.slots.get_mut(&done.key) else {
            return;
        };
        match (std::mem::replace(slot, Slot::Failed), done.result) {
            // 跳过了（后面又要了新尺寸）：不算在做，要的时候重新交给后台。
            (Slot::Pending(_), Outcome::Skipped) => {
                self.slots.remove(&done.key);
            }
            (Slot::Pending(job), Outcome::Drawn(drawn)) => {
                self.latest.insert(job.same, done.key);
                *slot = Slot::Ready {
                    rows: drawn.rows,
                    cols: drawn.protocol.size().width,
                    drawn: Some(drawn),
                    job,
                    redoing: false,
                    seen: frame,
                };
                self.revision += 1;
            }
            // 扔了编码、重做回来的（或者重做被跳过的）：占几行照旧，不重排。
            (
                Slot::Ready {
                    rows,
                    cols,
                    drawn,
                    job,
                    seen,
                    ..
                },
                outcome @ (Outcome::Drawn(_) | Outcome::Skipped),
            ) => {
                let drawn = match outcome {
                    Outcome::Drawn(fresh) => Some(fresh),
                    _ => drawn,
                };
                *slot = Slot::Ready {
                    rows,
                    cols,
                    drawn,
                    job,
                    redoing: false,
                    seen,
                };
            }
            (_, _) => self.revision += 1,
        }
        self.trim();
    }

    /// 编好的图记满了：扔这一帧没露出来的里面最久没露出来的，扔到不超过 `keep`；露着的不扔。
    fn trim(&mut self) {
        let frame = self.frame;
        loop {
            let held = self
                .slots
                .values()
                .filter(|s| matches!(s, Slot::Ready { drawn: Some(_), .. }))
                .count();
            if held <= self.keep {
                return;
            }
            let oldest = self
                .slots
                .iter()
                .filter_map(|(key, slot)| match slot {
                    Slot::Ready {
                        drawn: Some(_),
                        seen,
                        ..
                    } if *seen < frame => Some((*seen, *key)),
                    _ => None,
                })
                .min();
            let Some((_, key)) = oldest else {
                return;
            };
            if let Some(Slot::Ready { drawn, .. }) = self.slots.get_mut(&key) {
                *drawn = None;
            }
        }
    }
}

/// 是哪一张图，不管多大：种类、源码、写的宽高。
/// 这一种图最多多大（蓝图 `tui.md`「图片、公式和 mermaid 图」第 3 条）：图片缩得多；mermaid、公式、`<svg>`
/// 画的是字，缩多了看不清，照另一份。
pub fn room(look: &FigureLook, kind: FigureKind) -> Room {
    match kind {
        FigureKind::Image => look.picture_room,
        FigureKind::Mermaid | FigureKind::Math | FigureKind::Svg => look.room,
    }
}

fn same(kind: FigureKind, source: &str, size: Size) -> u64 {
    let mut hasher = DefaultHasher::new();
    (kind, source, size).hash(&mut hasher);
    hasher.finish()
}

/// 一张图的键：种类、源码、写的宽高、最多几列几行、换过几次主题（颜色烤在图里）。
fn key(kind: FigureKind, source: &str, size: Size, room: (u16, u16)) -> u64 {
    let mut hasher = DefaultHasher::new();
    (kind, source, size, room, theme::generation()).hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests;

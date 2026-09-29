//! 执行器的任务表（施工 7-3，`docs/blueprint/session/tools.md`「后台命令」，`agents.md` 第四条）：后台命令活过起它的那一次
//! 调用，由这里管。核心里一张（[`Jobs`]），所有会话共用：核心看它空不空闲。
//!
//! - 一个会话 actor 一份 [`SessionJobs`]：编号照日志往后数（[`JobIds`]，和派子代理的共用一串），每次调用造一个任务端口
//!   交给工具（`run.rs`）；
//! - 输出一直写进会话目录的 `jobs/<编号>.out`，不截；结束了存成 blob，把 `job.reported` 要的交回会话 actor，actor 交进
//!   内核、落了盘，才从表里拿掉：表空了，结束的记录一定都落了盘；
//! - 有计划地停下：在跑的各报一条 `restarted`，落了盘再整组杀；actor 因为别的停了（写不进去、panic、没人拿着了），整组
//!   杀掉、不记，再载入时内核补 `aborted`。

mod output;
mod run;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use tokio::sync::mpsc;

use miyu_kernel::event::{JobReason, JobReported};
use miyu_kernel::id::{CallId, CommandId, JobId};
use miyu_kernel::origin::{By, Tool};
use miyu_kernel::session::Input;
use miyu_kernel::time::Timestamp;
use miyu_store::blob::Blobs;
use miyu_tool::{JobPort, Process};

use crate::blocking::blocking;
use crate::job_ids::JobIds;
use crate::lines::millis;
use crate::port::Back;
use output::Output;
use run::Port;

/// 表里一项的钥匙：哪个会话 actor（一个 actor 一个号）的哪个任务。同一个会话停了又载入，前一个 actor 起的命令还没
/// 死透时编号可能重，actor 的号不会。
type Key = (u64, JobId);

/// 执行器的任务表：核心里一张，所有会话共用。
#[derive(Default)]
pub struct Jobs {
    table: Mutex<BTreeMap<Key, Entry>>,
    /// 下一个会话 actor 的号。
    owners: AtomicU64,
}

/// 表里的一条后台命令。
struct Entry {
    /// 它的进程：停下时整组杀。
    process: Arc<dyn Process>,
    /// 它的输出文件。
    output: Arc<Output>,
    /// 交给任务表的那一刻：用时从这里算。
    started: Instant,
    /// 结束已经有人报了：等着的那一头报了它自己退出，或者停下时报了 `restarted`。
    reported: bool,
}

impl Jobs {
    /// 一张空的表。
    pub fn new() -> Jobs {
        Jobs::default()
    }

    /// 有没有在跑的后台命令：结束了、记录还没落盘的也算。核心照它决定能不能空闲退出（`core.md`「停下」）。
    pub fn running(&self) -> bool {
        !self.lock().is_empty()
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<Key, Entry>> {
        self.table.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl std::fmt::Debug for Jobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jobs")
            .field("running", &self.lock().len())
            .finish()
    }
}

/// 一个会话 actor 的那一份。丢掉它（actor 停了）就把这个会话在跑的整组杀掉。
pub(crate) struct SessionJobs {
    shared: Arc<Shared>,
    /// 交进了内核、等这一批落了盘就从表里拿掉的。
    landing: Vec<Key>,
}

/// 会话 actor 和它造的每个任务端口共用的。
struct Shared {
    table: Arc<Jobs>,
    /// 这个 actor 的号。
    owner: u64,
    /// actor 还在：停了以后交来的命令不收，当场整组杀掉。拿着表的锁看、拿着表的锁改。
    open: AtomicBool,
    /// 会话目录：输出写在它下面的 `jobs/`。
    dir: PathBuf,
    /// 会话属主的 blob：结束了整份输出存进这里。
    blobs: Blobs,
    /// 任务编号：这个会话一串，和派子代理的共用。
    ids: Arc<JobIds>,
    /// 结束了交回 actor 的那一头。
    backs: mpsc::UnboundedSender<Back>,
}

/// 一条后台命令结束了，交回 actor 的：`job.reported` 的 `by`、`cause`、`body`（`kernel/session.md`「回报」第 2 条）。
#[derive(Debug)]
pub(crate) struct Ended {
    key: Key,
    by: By,
    cause: Option<CommandId>,
    reported: JobReported,
}

impl SessionJobs {
    /// 会话目录 `dir`、属主的 blob `blobs`，编号照这个会话的 `ids` 领（和派子代理的共用一串），结束了交回 `backs`。
    pub(crate) fn new(
        table: &Arc<Jobs>,
        dir: PathBuf,
        blobs: Blobs,
        ids: Arc<JobIds>,
        backs: mpsc::UnboundedSender<Back>,
    ) -> SessionJobs {
        let owner = table.owners.fetch_add(1, Ordering::Relaxed);
        SessionJobs {
            shared: Arc::new(Shared {
                table: Arc::clone(table),
                owner,
                open: AtomicBool::new(true),
                dir,
                blobs,
                ids,
                backs,
            }),
            landing: Vec::new(),
        }
    }

    /// 交给调用 `call_id` 的任务端口：它起的命令自己退出了，`by` 是这次调用，`cause` 是它所在那一轮的 `cause`。
    pub(crate) fn port(&self, call_id: CallId, cause: Option<CommandId>) -> Arc<dyn JobPort> {
        Arc::new(Port::new(
            Arc::clone(&self.shared),
            By::Tool(Tool { call_id }),
            cause,
        ))
    }

    /// 一条结束到了 actor：写成内核的输入，时刻是到的那一刻；记下它，这一批落了盘再从表里拿掉（[`Self::land`]）。
    pub(crate) fn arrived(&mut self, at: Timestamp, ended: Ended) -> Input {
        self.landing.push(ended.key);
        Input::JobEnded {
            at,
            by: ended.by,
            cause: ended.cause,
            reported: ended.reported,
        }
    }

    /// 交进内核的结束都落了盘：从表里拿掉。
    pub(crate) fn land(&mut self) {
        if self.landing.is_empty() {
            return;
        }
        let mut table = self.shared.table.lock();
        for key in self.landing.drain(..) {
            table.remove(&key);
        }
    }

    /// 有计划地停下（`agents.md` 第八条第 2 条）：这个会话在跑的，各写一条 `job.reported`（`restarted`，`by` 是内核），
    /// 带用时、到这时的输出。拿着表的锁把它们记成报了：之后它们自己退出了也不再报；在这以前自己退出、已经报了的，那一条
    /// 已经在 actor 的收件箱里了。杀掉在 [`Self::close`]，等这几条落了盘再做。
    pub(crate) async fn restarted(&self, at: Timestamp) -> Vec<Input> {
        let taken: Vec<(JobId, Arc<Output>, Instant)> = {
            let mut table = self.shared.table.lock();
            table
                .iter_mut()
                .filter(|((owner, _), entry)| *owner == self.shared.owner && !entry.reported)
                .map(|((_, job), entry)| {
                    entry.reported = true;
                    (*job, Arc::clone(&entry.output), entry.started)
                })
                .collect()
        };
        let blobs = self.shared.blobs.clone();
        let reports = blocking(move || {
            taken
                .into_iter()
                .map(|(job, output, started)| {
                    output.close();
                    let (output, chars) = output.stored(&blobs);
                    JobReported {
                        job,
                        reason: JobReason::Restarted,
                        exit_code: None,
                        signal: None,
                        by_model: false,
                        duration_ms: Some(millis(started.elapsed())),
                        output,
                        chars,
                    }
                })
                .collect::<Vec<_>>()
        })
        .await;
        reports
            .into_iter()
            .map(|reported| Input::JobEnded {
                at,
                by: By::Kernel,
                cause: None,
                reported,
            })
            .collect()
    }

    /// 停下：不再收新的，这个会话还在表里的都拿掉、整组杀掉。在阻塞线程里杀：Windows 上要等 `taskkill`。
    pub(crate) async fn close(&self) {
        let shared = Arc::clone(&self.shared);
        blocking(move || shared.close()).await;
    }
}

/// actor 停了：这个会话在跑的都整组杀掉。有计划地停下的已经杀过了，这里什么都没有。
impl Drop for SessionJobs {
    fn drop(&mut self) {
        self.shared.close();
    }
}

impl Shared {
    /// 不再收新的，这个会话还在表里的都拿掉、整组杀掉、不再写输出。已经结束了的，杀也不碍事（[`Process::kill`]）。
    fn close(&self) {
        let gone: Vec<Entry> = {
            let mut table = self.table.lock();
            self.open.store(false, Ordering::Release);
            let keys: Vec<Key> = table
                .keys()
                .filter(|(owner, _)| *owner == self.owner)
                .copied()
                .collect();
            keys.iter().filter_map(|key| table.remove(key)).collect()
        };
        for entry in gone {
            entry.output.close();
            entry.process.kill();
        }
    }

    /// 等着的那一头报它自己退出了：还在表里、还没人报过的，记成报了，交给 actor。拿着表的锁交：停下时拿到锁的那一刻，报了
    /// 的都已经在 actor 的收件箱里了。actor 已经停了的，从表里拿掉：没人会落它的盘了。
    fn report(&self, ended: Ended) {
        let mut table = self.table.lock();
        let Some(entry) = table.get_mut(&ended.key) else {
            return;
        };
        if entry.reported {
            return;
        }
        entry.reported = true;
        let key = ended.key;
        if self.backs.send(Back::Job(ended)).is_err() {
            table.remove(&key);
        }
    }
}

//! 扩展进程（施工 9-4 上，`docs/blueprint/extensions.md`）：核心拉起 `kind = "process"` 的包，经标准输入输出说同一套协议。
//!
//! 一个包一格：看管它的任务、任务报上来的状态。开关存在 `system/extensions.json`，没写的照清单的 `start`（`always` 的开着）。
//! 核心起来时照开关拉起开着的（[`Core::start_extensions`]），停的时候一起请它们退出（[`Core::stop_extensions`]）；有开着、没停下的，
//! 核心不算空闲。开、关、重启经协议（`methods.rs`），一件件办。看管一个包怎么拉起、退避、停下在 `supervise.rs`，标准错误的文件
//! 在 `stderr.rs`。

mod methods;
mod stderr;
mod supervise;

pub(crate) use methods::{disable, enable, restart, status};
pub use supervise::Timing;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::watch;
use tokio::task::JoinHandle;

use miyu_config::package::{Manifest, PackageKind, Start};
use miyu_store::config_file;
use miyu_store::extensions::{self as file, Switches};
use miyu_store::packages::locate;

use crate::Core;

/// 运行日志的目标。
const TARGET: &str = "miyu::extensions";

/// 一个扩展这时在哪一步。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    /// 关着。
    Off,
    /// 要拉起、拉起了，还没握手。
    Starting {
        /// 进程号：还没起来的没有。
        pid: Option<u32>,
    },
    /// 握了手。
    Running {
        /// 进程号。
        pid: u32,
    },
    /// 退避中：到点再拉起。
    Waiting {
        /// 哪一刻拉起。
        until: tokio::time::Instant,
    },
    /// 不再拉起：开关照旧开着，`restart` 再试，核心下次起来也再试。
    Stopped(Reason),
}

impl State {
    /// 协议里的写法。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            State::Off => "off",
            State::Starting { .. } => "starting",
            State::Running { .. } => "running",
            State::Waiting { .. } => "waiting",
            State::Stopped(_) => "stopped",
        }
    }

    /// 在干活、或者等着再干：核心不算空闲。
    fn busy(self) -> bool {
        matches!(
            self,
            State::Starting { .. } | State::Running { .. } | State::Waiting { .. }
        )
    }
}

/// 为什么停下了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reason {
    /// 退出码 1：配置错、端口被占这类重启也没用的。
    ConfigError,
    /// 连续失败到了上限。
    FailedRepeatedly,
    /// 程序不在 `miyu` 旁边。
    NotInstalled,
    /// 起不来：程序在，系统不让起、建不了它的目录和标准错误的文件。
    CannotStart,
    /// 清单说的协议版本不包含核心的。
    ProtocolMismatch,
}

impl Reason {
    /// 协议里的写法。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Reason::ConfigError => "config_error",
            Reason::FailedRepeatedly => "failed_repeatedly",
            Reason::NotInstalled => "not_installed",
            Reason::CannotStart => "cannot_start",
            Reason::ProtocolMismatch => "protocol_mismatch",
        }
    }
}

/// 一个扩展的状态和连续失败了几次：看管的任务写，列状态时读。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Status {
    /// 在哪一步。
    pub(crate) state: State,
    /// 连续失败了几次。
    pub(crate) failures: u32,
}

/// 表和看管的任务一起拿着的那一份。
type Shared = Arc<Mutex<Status>>;

/// 一个包一格。
struct Slot {
    /// 状态。
    status: Shared,
    /// 看管它的任务：请它停的开关、它本身。没拉起过、请停了的没有。
    task: Option<(watch::Sender<bool>, JoinHandle<()>)>,
}

/// 扩展的表：核心一份。
pub(crate) struct Extensions {
    /// 包的编号到它那一格。
    slots: Mutex<BTreeMap<String, Slot>>,
    /// 开、关、重启一件件办：两个连接同时开同一个，不拉起两个。
    ops: tokio::sync::Mutex<()>,
    /// 等多久、退避多久。
    timing: Timing,
}

impl Extensions {
    pub(crate) fn new(timing: Timing) -> Extensions {
        Extensions {
            slots: Mutex::new(BTreeMap::new()),
            ops: tokio::sync::Mutex::new(()),
            timing,
        }
    }

    fn slots(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Slot>> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 包 `id` 这时的状态：没拉起过的关着。
    pub(crate) fn status(&self, id: &str) -> Status {
        self.slots().get(id).map_or(
            Status {
                state: State::Off,
                failures: 0,
            },
            |slot| *slot.status.lock().unwrap_or_else(PoisonError::into_inner),
        )
    }

    /// 有在干活、或者等着再干的。
    pub(crate) fn busy(&self) -> bool {
        self.slots().values().any(|slot| {
            slot.status
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .state
                .busy()
        })
    }

    /// 拉起包 `id`（清单是 `manifest`）：在跑、在等的不动；程序没找到、协议版本对不上的直接停下。
    fn launch(&self, core: &Arc<Core>, id: &str, manifest: &Manifest) {
        let mut slots = self.slots();
        let slot = slots.entry(id.to_string()).or_insert_with(|| Slot {
            status: Arc::new(Mutex::new(Status {
                state: State::Off,
                failures: 0,
            })),
            task: None,
        });
        if slot
            .task
            .as_ref()
            .is_some_and(|(_, task)| !task.is_finished())
        {
            return;
        }
        let plan = match plan(core, id, manifest) {
            Ok(plan) => plan,
            Err(reason) => {
                tracing::warn!(target: TARGET, package = id, reason = reason.as_str(), "extension not started");
                *slot.status.lock().unwrap_or_else(PoisonError::into_inner) = Status {
                    state: State::Stopped(reason),
                    failures: 0,
                };
                slot.task = None;
                return;
            }
        };
        // 任务跑起来之前就算在干活：刚拉起的那一瞬间核心不算空闲。
        *slot.status.lock().unwrap_or_else(PoisonError::into_inner) = Status {
            state: State::Starting { pid: None },
            failures: 0,
        };
        let (stop, asked) = watch::channel(false);
        let task = tokio::spawn(supervise::run(
            Arc::downgrade(core),
            plan,
            Arc::clone(&slot.status),
            asked,
            self.timing,
        ));
        slot.task = Some((stop, task));
    }

    /// 请包 `id` 退出、等它退出（最多等到杀掉它），状态记成关着。
    async fn halt(&self, id: &str) {
        let task = self.slots().get_mut(id).and_then(|slot| slot.task.take());
        if let Some((stop, task)) = task {
            if stop.send(true).is_err() {
                // 任务自己停下了，没人收：不用管。
            }
            if let Err(error) = task.await {
                tracing::error!(target: TARGET, package = id, error = %error, "extension task failed");
            }
        }
        self.mark_off(id);
    }

    /// 请全部退出、一起等。
    async fn halt_all(&self) {
        let tasks: Vec<(String, JoinHandle<()>)> = self
            .slots()
            .iter_mut()
            .filter_map(|(id, slot)| {
                let (stop, task) = slot.task.take()?;
                if stop.send(true).is_err() {
                    // 任务自己停下了，没人收：不用管。
                }
                Some((id.clone(), task))
            })
            .collect();
        for (id, task) in tasks {
            if let Err(error) = task.await {
                tracing::error!(target: TARGET, package = id.as_str(), error = %error, "extension task failed");
            }
            self.mark_off(&id);
        }
    }

    /// 把包 `id` 记成关着、连续失败从零数。
    fn mark_off(&self, id: &str) {
        if let Some(slot) = self.slots().get(id) {
            *slot.status.lock().unwrap_or_else(PoisonError::into_inner) = Status {
                state: State::Off,
                failures: 0,
            };
        }
    }
}

impl Core {
    /// 照开关拉起开着的扩展（施工 9-4 上）：核心起来、开始接连接之前调一次。要在 tokio 的运行时里调。
    pub fn start_extensions(self: &Arc<Self>) {
        let (switches, _) = read_switches(self);
        for (id, manifest) in processes(self) {
            if on(&switches, id, manifest) {
                self.extensions.launch(self, id, manifest);
            }
        }
    }

    /// 请全部扩展退出，等它们退出（最多等到杀掉）：核心停的时候调。
    pub async fn stop_extensions(&self) {
        self.extensions.halt_all().await;
    }
}

/// 起来时读到的 `process` 包：编号和清单，照编号排。
fn processes(core: &Core) -> impl Iterator<Item = (&str, &Manifest)> {
    core.packages.iter().filter_map(|found| {
        let manifest = found.read.as_ref().ok()?;
        (manifest.kind == PackageKind::Process).then_some((found.id.as_str(), manifest))
    })
}

/// 包 `id` 开没开：写了的照写的，没写的照清单的 `start`。
fn on(switches: &Switches, id: &str, manifest: &Manifest) -> bool {
    switches.on.get(id).copied().unwrap_or_else(|| {
        manifest
            .process
            .as_ref()
            .is_some_and(|process| process.start == Start::Always)
    })
}

/// 开关的文件在哪。
fn switches_path(core: &Core) -> PathBuf {
    core.root.system().join(file::FILE)
}

/// 读开关：读不成的当没有，记一行 `WARN`；交回它的版本（写回时用），读不成的也交回整份字节的版本，好盖掉它。
fn read_switches(core: &Core) -> (Switches, Option<String>) {
    let path = switches_path(core);
    match file::read(&path) {
        Ok(read) => (read.switches, read.version),
        Err(error) => {
            tracing::warn!(target: TARGET, file = %path.display(), error = %error, "extension switches unreadable");
            let version = config_file::read(&path)
                .ok()
                .flatten()
                .map(|text| text.version);
            (Switches::default(), version)
        }
    }
}

/// 怎么拉起包 `id`：程序没找到、协议版本对不上的交回为什么。
fn plan(core: &Core, id: &str, manifest: &Manifest) -> Result<supervise::Plan, Reason> {
    if crate::packages::mismatch(manifest).is_some() {
        return Err(Reason::ProtocolMismatch);
    }
    let main = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu"));
    let program = manifest
        .command
        .as_ref()
        .and_then(|command| locate(&command.program, &main))
        .ok_or(Reason::NotInstalled)?;
    let args = manifest
        .process
        .as_ref()
        .map(|process| process.args.clone())
        .unwrap_or_default();
    Ok(supervise::Plan {
        id: id.to_string(),
        program,
        args,
        dir: crate::packages::packages(core).state_dir(id),
        log: stderr::path(core, id),
    })
}

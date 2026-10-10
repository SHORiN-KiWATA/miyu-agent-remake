//! 本机 embedding（施工 R-5 中，`docs/blueprint/recall.md` 第三条第 3 款、第四条第 3、4 款）：交给它一句话，交回一个向量。
//! 核心一份 [`Embedder`]，给要向量的一方（R-5 下接进 `memory_search`）。
//!
//! - **核对文件**（`embed/files.rs`，施工 R-5 三补）：模型文件在内置模型那个小程序包的目录里，装好就能用、不下。第一次要时在
//!   阻塞线程里照模型清单核对大小、SHA-256，同时来的等同一次（施工 R-5 五补：24 MB 十来毫秒，不先交「还在备」）；对不上的
//!   这一回用不了，不删包里的东西。
//! - **小程序**（`embed/worker.rs`）：要用时拉起 `miyu-embed`，一次一条，后来的排队；[`IDLE`] 没有新的请求就让它退出，下一条
//!   再拉起。它起不来、坏了，这一条交回 [`Unavailable::Failed`]；这一回起不来过三次，就不再拉起。
//! - 它不算核心「忙」：核心空闲退出照旧，它跟着没了（`worker.rs` 的 `Worker`）。
//! - **换**（施工 R-5 四补）：装卸内置模型那个包以后，向量那一路照新的清单另造一个换上（`Vectors::replace_local`），旧的
//!   [`Embedder::shut`]：标成用不了、小程序退出。别人手里的旧副本要的交回 [`Unavailable::Off`]，不再拉起。
//!
//! 远程的（`models.embedding` 写 `<供应商>/<模型>`，施工 R-5 补）在 `embed/remote.rs`：一次一个 HTTP 请求，不拉起小程序。

mod files;
mod remote;
mod worker;

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use miyu_recall::embedding::Manifest;

use crate::TARGET;
pub(crate) use remote::Remote;
use worker::{Failure, Worker};

/// 多久没有新的请求就让小程序退出（`recall.md` 第四条第 4 款）。
pub const IDLE: Duration = Duration::from_secs(600);

/// 这一回起不来几次，就不再拉起。
const TRIES: u32 = 3;

/// 造一个 [`Embedder`] 要的（核心照装了的内置模型那个小程序包拼，`miyu-core` 的 `embed::setup`，施工 R-5 三补）。几格都一样、
/// 模型清单的内容也一样的，换的时候算同一个（施工 R-5 四补）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedSetup {
    /// `miyu-embed` 在哪（照小程序清单的 `program`，在主程序真实位置的旁边找）；没找到的是空的。
    pub program: Option<PathBuf>,
    /// 模型清单的文件：包目录里的 `model.toml`。
    pub manifest: PathBuf,
    /// 模型的文件在哪个目录：包目录（`miyu_store::packages::Found::files_dir`）。
    pub dir: PathBuf,
    /// 多久没有新的请求就让小程序退出：照 [`IDLE`]，测试改短。
    pub idle: Duration,
}

/// 这一回算不出向量。原话是英文短句，进运行日志。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// 这台机器、这一阵用不了：没有小程序、模型清单读不了、包里的文件少了或对不上、起不来过三次；远程的那一家没配、地址或
    /// key 取不到。
    Off(String),
    /// 这一条算不出：小程序回了一句错、起不来、坏了；远程的发不出去、出错、超时、回的读不懂。下一条照常再试。
    Failed(String),
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unavailable::Off(why) | Unavailable::Failed(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Unavailable {}

/// 核心一份的本机 embedding。
#[derive(Debug, Clone)]
pub struct Embedder {
    shared: Arc<Shared>,
}

/// 照清单备好的、用得上的几样。
#[derive(Debug)]
struct Ready {
    program: PathBuf,
    manifest_path: PathBuf,
    manifest: Manifest,
    dir: PathBuf,
    idle: Duration,
}

#[derive(Debug)]
struct Shared {
    /// 照什么造的、那时读到的模型清单的原文（读不了的没有）：换的时候比（施工 R-5 四补）。
    made_from: (EmbedSetup, Option<String>),
    /// 用不用得上：用不上的是原因。
    ready: Result<Ready, String>,
    /// 关掉了（换下来了）：以后每一条都交回用不了，不再拉起（施工 R-5 四补）。
    shut: AtomicBool,
    /// 包里的文件核对过没有、对不对得上：只核对一次，同时来的等同一次（施工 R-5 五补）。对不上的是为什么。
    files: tokio::sync::OnceCell<Result<(), String>>,
    slot: tokio::sync::Mutex<Slot>,
}

/// 拉起着的小程序。
#[derive(Debug, Default)]
struct Slot {
    worker: Option<Worker>,
    /// 第几个拉起的：看空闲的那一个照它认是不是还在看自己拉起的那一个。
    generation: u64,
    /// 最近一条问完的时刻。
    used: Option<Instant>,
    /// 这一回起不来过几次。
    failures: u32,
}

impl Embedder {
    /// 照 `setup` 造。模型清单在这时读；用不上的（没有小程序、清单读不了）记一行 `WARN embedder unavailable`，以后每一条都交回
    /// [`Unavailable::Off`]。不核对文件、不拉起小程序：都等第一次要。
    pub fn new(setup: EmbedSetup) -> Embedder {
        let text = read(&setup.manifest);
        let ready = prepare(setup.clone(), &text);
        if let Err(reason) = &ready {
            tracing::warn!(target: TARGET, reason = reason.as_str(), "embedder unavailable");
        }
        Embedder {
            shared: Arc::new(Shared {
                made_from: (setup, text.ok()),
                ready,
                shut: AtomicBool::new(false),
                files: tokio::sync::OnceCell::new(),
                slot: tokio::sync::Mutex::new(Slot::default()),
            }),
        }
    }

    /// `text` 的向量（归一化过的，维数照清单）。一次一条：同时来的排队。
    ///
    /// # Errors
    ///
    /// 用不了（[`Unavailable::Off`]）；这一条算不出（[`Unavailable::Failed`]）。
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, Unavailable> {
        self.shared.open()?;
        let ready = self.shared.files_ready().await?;
        let mut slot = self.shared.slot.lock().await;
        // 排队的时候关掉的：拿到锁再看一次，不然会把换下来的又拉起。
        self.shared.open()?;
        if slot.failures >= TRIES {
            return Err(Unavailable::Off(format!(
                "miyu-embed failed to start {TRIES} times"
            )));
        }
        if slot.worker.is_none() {
            let started = Instant::now();
            match Worker::start(&ready.program, &ready.manifest_path, &ready.dir).await {
                Ok((worker, model)) => {
                    tracing::info!(
                        target: TARGET,
                        model = model.as_str(),
                        took_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                        "embedder started"
                    );
                    slot.worker = Some(worker);
                    slot.generation += 1;
                    watch_idle(Arc::downgrade(&self.shared), slot.generation, ready.idle);
                }
                Err(why) => {
                    slot.failures += 1;
                    tracing::warn!(target: TARGET, error = why.as_str(), "embedder failed");
                    return Err(Unavailable::Failed(why));
                }
            }
        }
        let Some(worker) = slot.worker.as_mut() else {
            return Err(Unavailable::Failed("miyu-embed is not running".to_string()));
        };
        let answered = worker.ask(text).await;
        slot.used = Some(Instant::now());
        match answered {
            Ok(vector) => Ok(vector),
            Err(Failure::Refused(why)) => Err(Unavailable::Failed(why)),
            Err(Failure::Broken(why)) => {
                tracing::warn!(target: TARGET, error = why.as_str(), "embedder failed");
                if let Some(worker) = slot.worker.take() {
                    worker.stop().await;
                }
                tracing::info!(target: TARGET, reason = "failed", "embedder stopped");
                Err(Unavailable::Failed(why))
            }
        }
    }

    /// 向量记的模型编号（`local:<id>`，施工 R-5 下）：用不上的（没有小程序、清单读不了）没有。
    pub fn model(&self) -> Option<String> {
        self.shared
            .ready
            .as_ref()
            .ok()
            .map(|ready| ready.manifest.model())
    }

    /// 本机清单的模型名（`id`，施工 R-5 再补）：设置页的「内置模型」后面暗字写它。用不上的没有。
    pub fn name(&self) -> Option<String> {
        self.shared
            .ready
            .as_ref()
            .ok()
            .map(|ready| ready.manifest.id.clone())
    }

    /// 小程序现在拉起着没有（给测试看；以后给头看状态）。
    pub async fn running(&self) -> bool {
        self.shared.slot.lock().await.worker.is_some()
    }

    /// 是不是照 `setup` 造的、模型清单的原文也没变（施工 R-5 四补）：换的时候一样的不动。读一次模型清单。
    pub(crate) fn serves(&self, setup: &EmbedSetup) -> bool {
        let (made, text) = &self.shared.made_from;
        made == setup && read(&setup.manifest).ok().as_ref() == text.as_ref()
    }

    /// 关掉（施工 R-5 四补）：以后每一条都交回用不了；拉起着的小程序等手里那一条问完、退出，退完才返回（最多等它一条的时限
    /// 加关掉的五秒）。记一行 `INFO embedder stopped reason=replaced`（拉起着的才记）。
    pub(crate) async fn shut(&self) {
        self.shared.shut.store(true, Ordering::SeqCst);
        let mut slot = self.shared.slot.lock().await;
        if let Some(worker) = slot.worker.take() {
            worker.stop().await;
            tracing::info!(target: TARGET, reason = "replaced", "embedder stopped");
        }
    }
}

impl Shared {
    /// 还开着：关掉了的交回用不了。
    fn open(&self) -> Result<(), Unavailable> {
        if self.shut.load(Ordering::SeqCst) {
            return Err(Unavailable::Off(
                "the embedding model was replaced".to_string(),
            ));
        }
        Ok(())
    }

    /// 文件齐了交回备好的几样：头一回在阻塞线程里核对，同时来的等同一次；对不上的交回用不了。
    async fn files_ready(self: &Arc<Self>) -> Result<&Ready, Unavailable> {
        let ready = self
            .ready
            .as_ref()
            .map_err(|why| Unavailable::Off(why.clone()))?;
        let shared = Arc::clone(self);
        let checked = self
            .files
            .get_or_init(|| async move {
                let checked = tokio::task::spawn_blocking(move || match &shared.ready {
                    Ok(ready) => files::check(&ready.manifest, &ready.dir)
                        .inspect_err(|why| {
                            tracing::warn!(target: TARGET, model = ready.manifest.id.as_str(), error = why.as_str(), "embedder unavailable");
                        }),
                    Err(why) => Err(why.clone()),
                })
                .await;
                checked.unwrap_or_else(|error| Err(format!("checking the model files failed: {error}")))
            })
            .await;
        checked
            .as_ref()
            .map(|()| ready)
            .map_err(|why| Unavailable::Off(why.clone()))
    }
}

/// 读模型清单的原文：读不了的交回原因。
fn read(manifest: &Path) -> Result<String, String> {
    std::fs::read_to_string(manifest)
        .map_err(|error| format!("cannot read {}: {error}", manifest.display()))
}

/// 照读到的模型清单原文 `text`、看有没有小程序：用不上的交回原因。
fn prepare(setup: EmbedSetup, text: &Result<String, String>) -> Result<Ready, String> {
    let program = setup
        .program
        .ok_or_else(|| "no miyu-embed next to the program".to_string())?;
    let text = text.as_ref().map_err(Clone::clone)?;
    let manifest = Manifest::parse(text).map_err(|error| error.to_string())?;
    Ok(Ready {
        program,
        dir: setup.dir,
        manifest_path: setup.manifest,
        manifest,
        idle: setup.idle,
    })
}

/// 看着第 `generation` 个拉起的小程序：`idle` 没有新的请求就关掉它。别的拉起来了、核心放下了这一份的，不再看。
fn watch_idle(shared: Weak<Shared>, generation: u64, idle: Duration) {
    tokio::spawn(async move {
        loop {
            let wait = {
                let Some(shared) = shared.upgrade() else {
                    return;
                };
                let mut slot = shared.slot.lock().await;
                if slot.generation != generation || slot.worker.is_none() {
                    return;
                }
                let since = slot.used.map_or(Duration::ZERO, |used| used.elapsed());
                if since >= idle {
                    if let Some(worker) = slot.worker.take() {
                        worker.stop().await;
                    }
                    tracing::info!(target: TARGET, reason = "idle", "embedder stopped");
                    return;
                }
                idle - since
            };
            tokio::time::sleep(wait).await;
        }
    });
}

#[cfg(test)]
mod tests;

//! 本机 embedding（施工 R-5 中，`docs/blueprint/recall.md` 第三条第 3 款、第四条第 3、4 款）：交给它一句话，交回一个向量。
//! 核心一份 [`Embedder`]，给要向量的一方（R-5 下接进 `memory_search`）。
//!
//! - **备齐文件**（`embed/fetch.rs`）：第一次要时在后台照清单核对、下载，放在缓存目录的 `embed/<id>/`。备着的时候交回
//!   [`Unavailable::Preparing`]，要的一方照只有关键词走，不等。下不成的，一小时以内不再试。
//! - **小程序**（`embed/worker.rs`）：要用时拉起 `miyu-embed`，一次一条，后来的排队；[`IDLE`] 没有新的请求就让它退出，下一条
//!   再拉起。它起不来、坏了，这一条交回 [`Unavailable::Failed`]；这一回起不来过三次，就不再拉起。
//! - 它不算核心「忙」：核心空闲退出照旧，它跟着没了（`worker.rs` 的 `Worker`）。

mod fetch;
mod worker;

use std::fmt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, Instant};

use miyu_http::Client;
use miyu_recall::embedding::Manifest;

use crate::TARGET;
use worker::{Failure, Worker};

/// 多久没有新的请求就让小程序退出（`recall.md` 第四条第 4 款）。
pub const IDLE: Duration = Duration::from_secs(600);

/// 下不成以后多久再试（照模型目录的 `RETRY`）。
const RETRY: Duration = Duration::from_secs(3600);

/// 这一回起不来几次，就不再拉起。
const TRIES: u32 = 3;

/// 造一个 [`Embedder`] 要的。
#[derive(Debug, Clone)]
pub struct EmbedSetup {
    /// `miyu-embed` 在哪（主程序真实位置的旁边）；没找到的是空的。
    pub program: Option<PathBuf>,
    /// 清单的文件（出厂的在资源目录的 `models/embed/`）。
    pub manifest: PathBuf,
    /// 放模型的目录：缓存目录下的 `embed`；缓存目录算不出的是空的。
    pub cache: Option<PathBuf>,
    /// 下载用的客户端（`miyu_http::fetcher`，代理照环境变量）。
    pub client: Client,
    /// 多久没有新的请求就让小程序退出：照 [`IDLE`]，测试改短。
    pub idle: Duration,
}

/// 这一回算不出向量。原话是英文短句，进运行日志。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// 模型还在核对、在下：这一回只有关键词。
    Preparing,
    /// 这台机器、这一阵用不了：没有小程序、没有缓存目录、清单读不了、下不成（一小时以后再试）、起不来过三次。
    Off(String),
    /// 这一条算不出：小程序回了一句错、起不来、坏了。下一条照常再试。
    Failed(String),
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unavailable::Preparing => write!(f, "the embedding model is being prepared"),
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
    client: Client,
    idle: Duration,
}

#[derive(Debug)]
struct Shared {
    /// 用不用得上：用不上的是原因。
    ready: Result<Ready, String>,
    files: Mutex<Files>,
    slot: tokio::sync::Mutex<Slot>,
}

/// 模型的文件备到哪了。
#[derive(Debug)]
enum Files {
    /// 这一回还没看过。
    Unchecked,
    /// 在后台核对、下载。
    Preparing,
    /// 齐了、核对过。
    Ready,
    /// 没备成：什么时候、为什么。
    Failed { at: Instant, why: String },
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
    /// 照 `setup` 造。清单在这时读；用不上的（没有小程序、没有缓存目录、清单读不了）记一行 `WARN embedder unavailable`，
    /// 以后每一条都交回 [`Unavailable::Off`]。不碰网络、不拉起小程序：都等第一次要。
    pub fn new(setup: EmbedSetup) -> Embedder {
        let ready = prepare(setup);
        if let Err(reason) = &ready {
            tracing::warn!(target: TARGET, reason = reason.as_str(), "embedder unavailable");
        }
        Embedder {
            shared: Arc::new(Shared {
                ready,
                files: Mutex::new(Files::Unchecked),
                slot: tokio::sync::Mutex::new(Slot::default()),
            }),
        }
    }

    /// `text` 的向量（归一化过的，维数照清单）。一次一条：同时来的排队。
    ///
    /// # Errors
    ///
    /// 模型还在备（[`Unavailable::Preparing`]）；用不了（[`Unavailable::Off`]）；这一条算不出（[`Unavailable::Failed`]）。
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, Unavailable> {
        let ready = self.shared.files_ready()?;
        let mut slot = self.shared.slot.lock().await;
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

    /// 向量记的模型编号（`local:<id>`，施工 R-5 下）：用不上的（没有小程序、缓存目录、清单）没有。
    pub fn model(&self) -> Option<String> {
        self.shared
            .ready
            .as_ref()
            .ok()
            .map(|ready| ready.manifest.model())
    }

    /// 小程序现在拉起着没有（给测试看；以后给头看状态）。
    pub async fn running(&self) -> bool {
        self.shared.slot.lock().await.worker.is_some()
    }
}

impl Shared {
    /// 文件齐了交回备好的几样；没看过的在后台开始备，交回「还在备」。
    fn files_ready(self: &Arc<Self>) -> Result<&Ready, Unavailable> {
        let ready = self
            .ready
            .as_ref()
            .map_err(|why| Unavailable::Off(why.clone()))?;
        let mut files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
        match &*files {
            Files::Ready => return Ok(ready),
            Files::Preparing => return Err(Unavailable::Preparing),
            Files::Failed { at, why } if at.elapsed() < RETRY => {
                return Err(Unavailable::Off(why.clone()));
            }
            Files::Unchecked | Files::Failed { .. } => {}
        }
        *files = Files::Preparing;
        let shared = Arc::clone(self);
        tokio::spawn(async move {
            let Ok(ready) = &shared.ready else { return };
            let prepared = fetch::prepare(&ready.manifest, &ready.dir, &ready.client).await;
            if let Err(why) = &prepared {
                tracing::warn!(target: TARGET, model = ready.manifest.id.as_str(), error = why.as_str(), "embedding model not downloaded");
            }
            let mut files = shared.files.lock().unwrap_or_else(PoisonError::into_inner);
            *files = match prepared {
                Ok(()) => Files::Ready,
                Err(why) => Files::Failed {
                    at: Instant::now(),
                    why,
                },
            };
        });
        Err(Unavailable::Preparing)
    }
}

/// 读清单、看有没有小程序和缓存目录：用不上的交回原因。
fn prepare(setup: EmbedSetup) -> Result<Ready, String> {
    let program = setup
        .program
        .ok_or_else(|| "no miyu-embed next to the program".to_string())?;
    let cache = setup
        .cache
        .ok_or_else(|| "no cache directory".to_string())?;
    let text = std::fs::read_to_string(&setup.manifest)
        .map_err(|error| format!("cannot read {}: {error}", setup.manifest.display()))?;
    let manifest = Manifest::parse(&text).map_err(|error| error.to_string())?;
    Ok(Ready {
        program,
        dir: fetch::dir_of(&cache, &manifest),
        manifest_path: setup.manifest,
        manifest,
        client: setup.client,
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

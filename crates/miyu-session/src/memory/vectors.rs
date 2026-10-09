//! 记忆、以前的对话照意思找的那一路（施工 R-5 下，`docs/blueprint/recall.md` 第三条）：算问句的向量，在后台补检索库里缺的。
//!
//! - 照哪一路算（施工 R-5 补）：照调的一方手里的配置（[`Using`]）的 `models.embedding`。不写、写 `local` 的照本机的
//!   （`embed.rs`），写 `<供应商>/<模型>` 的照那一家（`embed/remote.rs`），`off` 的不算问句、不补。向量照模型的编号存：本机的
//!   是 `local:<id>`，远程的是 `<供应商>/<模型>`；换了的照新的补，旧的留着。
//! - 问句：最多等 [`WAIT`]（第一次拉起小程序要一两百毫秒）。等不到的这一回只走关键词；那一条照样在后台算完，不把小程序的
//!   一问一答打断（打断了它回的那一行就对不上下一问）。
//! - 补：搜的时候起。照这一间的记忆库、回合库各起一个后台的，一次取 [`BATCH`] 条还没有这个模型的向量的，一条一条算、写回；
//!   模型还在备的等它备好；这一回算不出的那几条跳过（照行号往后取，不挡住后面的），下次再补；连着 [`STREAK`] 条算不出的这一回
//!   不补了（远程那一家挂了、key 错了，不一条一条地等）。同一份库同一时刻只有一个在补。

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use miyu_kernel::id::AccountId;
use miyu_models::settings::UseSettings;
use miyu_store::memory::MemoryLog;
use miyu_store::recall::RecallIndex;

use crate::TARGET;
use crate::blocking::blocking;
use crate::config::TurnConfig;
use crate::embed::{Embedder, Remote, Unavailable};
use crate::route::shared::ModelData;

/// 问句的向量最多等多久。
const WAIT: Duration = Duration::from_secs(1);

/// 补的时候一次取几条。
const BATCH: usize = 16;

/// 模型还在备的时候隔多久再问一次。
const PREPARING: Duration = Duration::from_secs(1);

/// 补的时候连着几条算不出，这一回就不补了。
const STREAK: u32 = 3;

/// `models.embedding` 照本机的写法。
const LOCAL: &str = "local";

/// `models.embedding` 关掉的写法。
const OFF: &str = "off";

/// 照什么算向量：调的一方手里的配置（她的工具是这一轮冻结的，协议是这时的），远程的花钱记在谁的账上。
#[derive(Debug, Clone)]
pub struct Using {
    /// 照它的 `models.embedding` 挑哪一路，远程的照它取地址、key。
    pub config: TurnConfig,
    /// 远程的用量记在谁名下：会话的属主，协议是连上来的那个账号。
    pub owner: AccountId,
}

/// 问句的向量：哪个模型算的、向量。
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// 模型编号（`local:<id>`、`<供应商>/<模型>`）：只和这个模型的向量比。
    pub model: String,
    /// 归一化过的向量。
    pub vector: Vec<f32>,
}

/// 要补的一份库。
#[derive(Debug, Clone)]
pub(crate) enum Target {
    /// 这一间的回合库。
    Turns(Arc<RecallIndex>),
    /// 这一间的记忆库（在记忆日志里）。
    Memories(Arc<MemoryLog>),
}

impl Target {
    fn index(&self) -> &RecallIndex {
        match self {
            Target::Turns(index) => index,
            Target::Memories(log) => log.index(),
        }
    }
}

/// 这一回照哪一路算。
enum Way {
    /// 本机的小程序。
    Local(Embedder),
    /// 远程的那一家。
    Remote(Remote),
}

impl Way {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, Unavailable> {
        match self {
            Way::Local(embedder) => embedder.embed(text).await,
            Way::Remote(remote) => remote.embed(text).await,
        }
    }
}

/// 核心一份的向量那一路：本机算向量的、远程查供应商要的模型资料、在补的那几份库。
#[derive(Debug)]
pub struct Vectors {
    local: Option<Embedder>,
    data: Arc<ModelData>,
    /// 在补的：库的名字（[`Target`] 的种类加这一间）。
    filling: Mutex<BTreeSet<String>>,
}

impl Vectors {
    /// 本机的照 `local` 算（找不到小程序、缓存目录的没有），远程的照模型资料 `data` 里的供应商发、记账。
    pub fn new(local: Option<Embedder>, data: Arc<ModelData>) -> Vectors {
        Vectors {
            local,
            data,
            filling: Mutex::new(BTreeSet::new()),
        }
    }

    /// 照 `using` 挑：模型编号和算的那一路。`off` 的、要本机的却没有的没有。
    fn way(&self, using: &Using) -> Option<(String, Way)> {
        let values = using.config.resolved.values();
        match UseSettings::from(&values).embedding.as_deref() {
            Some(OFF) => None,
            None | Some(LOCAL) => {
                let local = self.local.clone()?;
                Some((local.model()?, Way::Local(local)))
            }
            // 别的都是 `<供应商>/<模型>`：配置照 `model_or` 查过写法。
            Some(written) => {
                let (provider, model) = written.split_once('/')?;
                let remote = Remote {
                    data: Arc::clone(&self.data),
                    config: Arc::clone(&using.config),
                    owner: using.owner.clone(),
                    provider: provider.to_string(),
                    model: model.to_string(),
                };
                Some((written.to_string(), Way::Remote(remote)))
            }
        }
    }

    /// 照 `using` 算问句 `text` 的向量，最多等 1 秒；`off` 的、还在备、用不了、算不出、等不到的没有。
    pub async fn query(&self, using: &Using, text: &str) -> Option<Query> {
        let (model, way) = self.way(using)?;
        let text = text.to_string();
        let asked = tokio::spawn(async move { way.embed(&text).await });
        match tokio::time::timeout(WAIT, asked).await {
            Ok(Ok(Ok(vector))) => Some(Query { model, vector }),
            Ok(Ok(Err(why))) => {
                tracing::debug!(target: TARGET, reason = %why, "query not embedded");
                None
            }
            Ok(Err(_)) | Err(_) => {
                tracing::debug!(target: TARGET, "query not embedded in time");
                None
            }
        }
    }

    /// 照 `using` 在后台补 `target` 里缺的向量（`off` 的不补）；`name` 是它的名字，同一个名字同一时刻只有一个在补。
    pub(crate) fn fill(self: &Arc<Self>, using: &Using, name: String, target: Target) {
        let Some((model, way)) = self.way(using) else {
            return;
        };
        {
            let mut filling = self.filling.lock().unwrap_or_else(PoisonError::into_inner);
            if !filling.insert(name.clone()) {
                return;
            }
        }
        let vectors = Arc::clone(self);
        tokio::spawn(async move {
            let started = Instant::now();
            let count = fill_all(&way, &model, &target).await;
            if count > 0 {
                tracing::info!(
                    target: TARGET,
                    index = name.as_str(),
                    count,
                    took_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                    "memory vectors filled"
                );
            }
            vectors
                .filling
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&name);
        });
    }
}

/// 一批一批补到没有缺的：交回补了几条。
async fn fill_all(way: &Way, model: &str, target: &Target) -> usize {
    let mut after = 0;
    let mut count = 0;
    let mut failed = 0;
    loop {
        let (asked, model_of) = (target.clone(), model.to_string());
        let batch = blocking(move || asked.index().missing(&model_of, after, BATCH)).await;
        let batch = match batch {
            Ok(batch) if batch.is_empty() => return count,
            Ok(batch) => batch,
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "memory vectors not filled");
                return count;
            }
        };
        for (row, key, text) in batch {
            after = row;
            let vector = match vector_of(way, &text).await {
                Got::Vector(vector) => vector,
                Got::Skip => {
                    failed += 1;
                    if failed >= STREAK {
                        return count;
                    }
                    continue;
                }
                Got::Stop => return count,
            };
            failed = 0;
            let (put, model_of) = (target.clone(), model.to_string());
            let stored = blocking(move || put.index().put_vector(&key, &model_of, &vector)).await;
            if let Err(error) = stored {
                tracing::warn!(target: TARGET, error = %error, "memory vectors not filled");
                return count;
            }
            count += 1;
        }
    }
}

/// 一条的向量：模型还在备的等它备好。
async fn vector_of(way: &Way, text: &str) -> Got {
    loop {
        match way.embed(text).await {
            Ok(vector) => return Got::Vector(vector),
            Err(Unavailable::Preparing) => tokio::time::sleep(PREPARING).await,
            Err(Unavailable::Failed(_)) => return Got::Skip,
            Err(Unavailable::Off(_)) => return Got::Stop,
        }
    }
}

/// 补一条的结果。
enum Got {
    /// 算出来了。
    Vector(Vec<f32>),
    /// 这一条算不出：跳过，接着补别的。
    Skip,
    /// 这一阵用不了（下不成、起不来过三次，远程的那一家没配）：这一回不补了。
    Stop,
}

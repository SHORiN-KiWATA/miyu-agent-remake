//! 记忆、以前的对话照意思找的那一路（施工 R-5 下，`docs/blueprint/recall.md` 第三条）：算问句的向量，在后台补检索库里缺的。
//!
//! - 问句：最多等 [`WAIT`]（第一次拉起小程序要一两百毫秒）。等不到的这一回只走关键词；那一条照样在后台算完，不把小程序的
//!   一问一答打断（打断了它回的那一行就对不上下一问）。
//! - 补：搜的时候起。照这一间的记忆库、回合库各起一个后台的，一次取 [`BATCH`] 条还没有这个模型的向量的，一条一条算、写回；
//!   模型还在备的等它备好；这一回算不出的那几条跳过（照行号往后取，不挡住后面的），下次再补。同一份库同一时刻只有一个在补。
//! - `models.embedding` 是 `off` 的不算问句、不补（调的一方照这一轮的配置判）。

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use miyu_config::Values;
use miyu_models::settings::UseSettings;
use miyu_store::memory::MemoryLog;
use miyu_store::recall::RecallIndex;

use crate::TARGET;
use crate::blocking::blocking;
use crate::embed::{Embedder, Unavailable};

/// 问句的向量最多等多久。
const WAIT: Duration = Duration::from_secs(1);

/// 补的时候一次取几条。
const BATCH: usize = 16;

/// 模型还在备的时候隔多久再问一次。
const PREPARING: Duration = Duration::from_secs(1);

/// `models.embedding` 关掉的写法。
const OFF: &str = "off";

/// 这一份配置照不照意思找记忆（施工 R-5 下）：`models.embedding` 不是 `off` 就照（不写的照 `local`）。她的工具照这一轮的
/// 配置，协议照这时的配置。
pub fn by_meaning(values: &Values) -> bool {
    UseSettings::from(values).embedding.as_deref() != Some(OFF)
}

/// 问句的向量：哪个模型算的、向量。
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    /// 模型编号（`local:<id>`）：只和这个模型的向量比。
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

/// 核心一份的向量那一路：算向量的、在补的那几份库。
#[derive(Debug)]
pub struct Vectors {
    embedder: Embedder,
    /// 在补的：库的名字（[`Target`] 的种类加这一间）。
    filling: Mutex<BTreeSet<String>>,
}

impl Vectors {
    /// 照 `embedder` 算。
    pub fn new(embedder: Embedder) -> Vectors {
        Vectors {
            embedder,
            filling: Mutex::new(BTreeSet::new()),
        }
    }

    /// 问句 `text` 的向量，最多等 1 秒；还在备、用不了、算不出、等不到的没有。
    pub async fn query(&self, text: &str) -> Option<Query> {
        let model = self.embedder.model()?;
        let embedder = self.embedder.clone();
        let text = text.to_string();
        let asked = tokio::spawn(async move { embedder.embed(&text).await });
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

    /// 在后台补 `target` 里缺的向量；`name` 是它的名字，同一个名字同一时刻只有一个在补。
    pub(crate) fn fill(self: &Arc<Self>, name: String, target: Target) {
        let Some(model) = self.embedder.model() else {
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
            let count = vectors.fill_all(&model, &target).await;
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

    /// 一批一批补到没有缺的：交回补了几条。
    async fn fill_all(&self, model: &str, target: &Target) -> usize {
        let mut after = 0;
        let mut count = 0;
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
                let vector = match self.vector_of(&text).await {
                    Got::Vector(vector) => vector,
                    Got::Skip => continue,
                    Got::Stop => return count,
                };
                let (put, model_of) = (target.clone(), model.to_string());
                let stored =
                    blocking(move || put.index().put_vector(&key, &model_of, &vector)).await;
                if let Err(error) = stored {
                    tracing::warn!(target: TARGET, error = %error, "memory vectors not filled");
                    return count;
                }
                count += 1;
            }
        }
    }

    /// 一条的向量：模型还在备的等它备好。
    async fn vector_of(&self, text: &str) -> Got {
        loop {
            match self.embedder.embed(text).await {
                Ok(vector) => return Got::Vector(vector),
                Err(Unavailable::Preparing) => tokio::time::sleep(PREPARING).await,
                Err(Unavailable::Failed(_)) => return Got::Skip,
                Err(Unavailable::Off(_)) => return Got::Stop,
            }
        }
    }
}

/// 补一条的结果。
enum Got {
    /// 算出来了。
    Vector(Vec<f32>),
    /// 这一条算不出：跳过，接着补别的。
    Skip,
    /// 这一阵用不了（下不成、起不来过三次）：这一回不补了。
    Stop,
}

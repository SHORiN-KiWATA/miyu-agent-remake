//! 后台更新目录（`docs/blueprint/models.md`「怎么走」第二条第 3 条，施工 8-7）：`models.catalog.update` 开着的，缓存的
//! `fetched` 旧过 `every` 的，GET `url`，带上次的 `ETag`，连接 10 秒、整个 60 秒、最多 32 MiB。
//!
//! - 200：先读一遍，至少有一家供应商才算好的；写进缓存目录（临时文件再改名，`meta` 一起），换上新的，记
//!   `INFO catalog refreshed`。304：只改 `meta` 的 `fetched`，记 `INFO catalog not modified`。
//! - 别的、读不了的、写不进的：记 `WARN catalog refresh failed`，一小时后再试。
//! - 核心一直开着的，每过 `every` 再查一次；配置改了（当场生效）照新的算。缓存目录算不出来的不拉。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::watch;

use miyu_http::{Client, Get, Got, get};
use miyu_kernel::time::Timestamp;
use miyu_models::catalog::{Catalog, CatalogSource, Loaded};
use miyu_models::settings::CatalogSettings;
use miyu_session::ModelData;

use super::catalog::{FILE, META, Meta};
use crate::TARGET;

/// 失败了多久以后再试。
pub const RETRY: Duration = Duration::from_secs(60 * 60);

/// 整个最多多久。
const TIMEOUT: Duration = Duration::from_secs(60);

/// 最多多少字节。
const LIMIT: usize = 32 * 1024 * 1024;

/// 后台更新要的：交给谁、用哪个客户端、缓存在哪（`<缓存目录>/models`）。
#[derive(Clone)]
pub struct Refresher {
    /// 换上新目录的地方。
    pub data: Arc<ModelData>,
    /// GET 用的客户端（`miyu_http::fetcher`）。
    pub client: Client,
    /// 缓存的目录。
    pub cache: PathBuf,
}

/// 拉一次的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refreshed {
    /// 拿到了新的，换上了。
    Replaced,
    /// 和缓存里的一样。
    NotModified,
    /// 没成，记过一行。
    Failed,
}

/// 还要等多久再拉：`fetched` 是缓存上次拉的时刻（没有的马上拉），`failed` 是上次失败的时刻（有的照一小时后）。
pub fn next_try(
    now: SystemTime,
    fetched: Option<SystemTime>,
    every: Duration,
    failed: Option<SystemTime>,
) -> Duration {
    let due = match failed {
        Some(failed) => failed + RETRY,
        None => fetched.map_or(now, |fetched| fetched + every),
    };
    due.duration_since(now).unwrap_or(Duration::ZERO)
}

impl Refresher {
    /// 一直跑：照 `settings`（配置改了当场换）算什么时候拉，到了拉一次。`settings` 的发送方没了就停。
    pub async fn run(self, mut settings: watch::Receiver<CatalogSettings>) {
        let mut failed = None;
        loop {
            let now = settings.borrow_and_update().clone();
            let wait = now.update.then(|| {
                let fetched = Meta::read(&self.cache).and_then(|meta| time(&meta.fetched));
                next_try(SystemTime::now(), fetched, now.every, failed)
            });
            let due = match wait {
                Some(wait) => tokio::select! {
                    () = tokio::time::sleep(wait) => true,
                    changed = settings.changed() => match changed {
                        Ok(()) => false,
                        Err(_) => return,
                    },
                },
                None => match settings.changed().await {
                    Ok(()) => false,
                    Err(_) => return,
                },
            };
            if due {
                failed = match self.once(&now.url).await {
                    Refreshed::Failed => Some(SystemTime::now()),
                    Refreshed::Replaced | Refreshed::NotModified => None,
                };
            }
        }
    }

    /// 拉一次 `url`。
    pub async fn once(&self, url: &str) -> Refreshed {
        match self.fetch(url).await {
            Ok(refreshed) => refreshed,
            Err(error) => {
                tracing::warn!(target: TARGET, error = %error, "catalog refresh failed");
                Refreshed::Failed
            }
        }
    }

    async fn fetch(&self, url: &str) -> Result<Refreshed, String> {
        let meta = Meta::read(&self.cache);
        // 缓存的目录本体还在才带 `ETag`：不然 304 了手里什么都没有。
        let etag = meta
            .as_ref()
            .filter(|_| self.cache.join(FILE).is_file())
            .and_then(|meta| meta.etag.clone());
        let got = get(Get {
            client: &self.client,
            url,
            headers: &[],
            etag: etag.as_deref(),
            timeout: TIMEOUT,
            limit: LIMIT,
        })
        .await?;
        let fetched = Timestamp::from_unix_millis(now_millis())
            .map(|at| at.to_string())
            .ok_or("the clock is out of range")?;
        match got {
            Got::NotModified => {
                let mut meta = meta.ok_or("not modified without a cached copy")?;
                meta.fetched = fetched;
                write(&self.cache, META, meta.to_json().as_bytes()).await?;
                tracing::info!(target: TARGET, "catalog not modified");
                Ok(Refreshed::NotModified)
            }
            Got::Body { bytes, etag } => {
                let text =
                    String::from_utf8(bytes).map_err(|_| "catalog is not UTF-8".to_string())?;
                let catalog = Catalog::parse(&text)?.catalog;
                if catalog.providers().next().is_none() {
                    return Err("catalog has no providers".to_string());
                }
                let meta = Meta {
                    source: url.to_string(),
                    fetched: fetched.clone(),
                    etag,
                };
                write(&self.cache, FILE, text.as_bytes()).await?;
                write(&self.cache, META, meta.to_json().as_bytes()).await?;
                tracing::info!(
                    target: TARGET,
                    fetched = %fetched,
                    providers = catalog.providers().count(),
                    models = catalog.model_count(),
                    "catalog refreshed"
                );
                self.data.replace_catalog(Loaded {
                    catalog,
                    source: CatalogSource::Cache,
                    fetched,
                });
                Ok(Refreshed::Replaced)
            }
        }
    }
}

/// 在阻塞线程里写 `dir/name`：先写临时文件再改名。
async fn write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let (path, bytes) = (dir.join(name), bytes.to_vec());
    tokio::task::spawn_blocking(move || miyu_store::generated::write(&path, &bytes))
        .await
        .map_err(|error| error.to_string())?
        .map(|_| ())
        .map_err(|error| format!("{name} not written: {error}"))
}

/// `meta` 里的时刻；读不懂的当没有。
fn time(text: &str) -> Option<SystemTime> {
    let millis = u64::try_from(Timestamp::parse(text).ok()?.unix_millis()).ok()?;
    Some(UNIX_EPOCH + Duration::from_millis(millis))
}

/// 此刻，Unix 毫秒。
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
        })
}

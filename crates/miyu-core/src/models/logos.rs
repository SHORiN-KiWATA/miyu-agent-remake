//! 供应商的图标拉到本机（施工 8-31，`docs/blueprint/models.md`「图标」）：头不连外网，也不让外网知道用的是哪一家，所以核心
//! 把目录里每一家的都拉下来，存进缓存目录 `<缓存目录>/models/logos/`，交给 [`ModelData`]，`provider.catalog`、`model.list`
//! 照它带上 `logo`。
//!
//! - 节奏同目录（`[models.catalog]` 的 `update`、`every`）：上次拉完旧过 `every` 的拉一次，关了的不拉、只读缓存；失败的一小时
//!   以后再试。核心起来时先把缓存里的换上。
//! - 一次拉：先照 models.dev 不存在的编号拉一张记下默认图（拉不到的这一次不拉，认不出默认图），再把目录里每一家的照
//!   `models/logos.json` 拉，同时最多 [`PARALLEL`] 张，每张最多 [`LOGO_MAX`] 字节、10 秒；收不收照
//!   [`miyu_models::logos::accept`]。拉不到的当没有。
//! - 存：一家一个文件，单色的 `<编号>.svg`，彩色的 `<编号>.color.svg`；拉完写 `fetched`（这一刻），整份换掉上次的，记
//!   `INFO logos refreshed`。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use tokio::sync::{Semaphore, watch};
use tokio::task::JoinSet;

use miyu_http::{Client, Get, Got, get};
use miyu_kernel::time::Timestamp;
use miyu_models::logos::{LOGO_MAX, Logo, LogoTable, accept};
use miyu_session::ModelData;

use super::refresh::{RETRY, Schedule, next_try};
use crate::TARGET;

/// 同时最多拉几张。
pub const PARALLEL: usize = 8;

/// 一张最多等多久。
const TIMEOUT: Duration = Duration::from_secs(10);

/// 记上次拉完的时刻的文件。
const FETCHED: &str = "fetched";

/// 出厂的对照表在资源目录 `models/` 下的名字。
pub const TABLE: &str = "logos.json";

/// 读出厂的对照表 `path`；读不了、写错了的记一行 `WARN logo table unreadable`，交回没有（不拉图标）。
pub fn table(path: &Path) -> Option<LogoTable> {
    let read = std::fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|text| LogoTable::parse(&text));
    match read {
        Ok(table) => Some(table),
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "logo table unreadable");
            None
        }
    }
}

/// 拉图标要的。
pub struct Logos {
    /// 换上图标的地方，也从它拿目录里有哪几家。
    pub data: Arc<ModelData>,
    /// GET 用的客户端，和拉目录的同一个。
    pub client: Client,
    /// `<缓存目录>/models/logos`。
    pub dir: PathBuf,
    /// 出厂的 `models/logos.json`。
    pub table: LogoTable,
}

impl Logos {
    /// 一直跑：先换上缓存里的，再照 `settings`（同目录）到点拉。`settings` 的发送方没了就停。
    pub async fn run(self, mut settings: watch::Receiver<Schedule>) {
        let dir = self.dir.clone();
        match tokio::task::spawn_blocking(move || load(&dir)).await {
            Ok(cached) => self.data.set_logos(cached),
            Err(error) => tracing::error!(target: TARGET, error = %error, "logos read panicked"),
        }
        let mut failed = None;
        loop {
            let now = settings.borrow_and_update().clone();
            let wait = now
                .update
                .then(|| next_try(SystemTime::now(), fetched(&self.dir), now.every, failed));
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
                failed = match self.once().await {
                    Ok(count) => {
                        tracing::info!(target: TARGET, logos = count, "logos refreshed");
                        None
                    }
                    Err(error) => {
                        tracing::warn!(target: TARGET, error = %error, retry_secs = RETRY.as_secs(), "logos refresh failed");
                        Some(SystemTime::now())
                    }
                };
            }
        }
    }

    /// 拉一次，交回收下了几家。
    ///
    /// # Errors
    ///
    /// 目录里没有供应商、默认图拉不到、存不进缓存的，说哪里不对；这一次什么都不换。
    pub async fn once(&self) -> Result<usize, String> {
        self.data.wait().await;
        let ids: Vec<String> = self
            .data
            .catalog()
            .map(|loaded| {
                loaded
                    .catalog
                    .providers()
                    .map(|provider| provider.id.clone())
                    .filter(|id| file_name(id))
                    .collect()
            })
            .unwrap_or_default();
        if ids.is_empty() {
            return Err("the catalog has no providers".to_string());
        }
        let default = fetch(&self.client, &self.table.probe()).await?;
        let gate = Arc::new(Semaphore::new(PARALLEL));
        let mut running = JoinSet::new();
        for id in ids {
            let (url, tint) = self.table.source(&id);
            let (client, gate, default) = (self.client.clone(), Arc::clone(&gate), default.clone());
            running.spawn(async move {
                let _held = gate.acquire_owned().await.ok()?;
                let bytes = fetch(&client, &url).await.ok()?;
                let svg = accept(&bytes, tint.then_some(default.as_slice()))?;
                Some((id, Logo { svg, tint }))
            });
        }
        let mut logos = BTreeMap::new();
        while let Some(done) = running.join_next().await {
            if let Ok(Some((id, logo))) = done {
                logos.insert(id, logo);
            }
        }
        let (dir, kept) = (self.dir.clone(), logos.clone());
        tokio::task::spawn_blocking(move || store(&dir, &kept))
            .await
            .map_err(|error| error.to_string())??;
        let count = logos.len();
        self.data.set_logos(logos);
        Ok(count)
    }
}

/// GET 一张，交回 2xx 的正文。
async fn fetch(client: &Client, url: &str) -> Result<Vec<u8>, String> {
    match get(Get {
        client,
        url,
        headers: &[],
        etag: None,
        timeout: TIMEOUT,
        limit: LOGO_MAX,
    })
    .await?
    {
        Got::Body { bytes, .. } => Ok(bytes),
        Got::NotModified => Err(format!("{url}: not modified without asking")),
    }
}

/// 编号能不能当文件名：目录里的编号都是小写字母、数字、`-`、`_`，别的不拉。
fn file_name(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// 读缓存里的图标：`<编号>.svg` 单色、`<编号>.color.svg` 彩色，收不收照样查一遍。目录读不了的当没有。
pub fn load(dir: &Path) -> BTreeMap<String, Logo> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return BTreeMap::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let (id, tint) = match name.strip_suffix(".color.svg") {
                Some(id) => (id, false),
                None => (name.strip_suffix(".svg")?, true),
            };
            let bytes = std::fs::read(entry.path()).ok()?;
            let svg = accept(&bytes, None)?;
            file_name(id).then(|| (id.to_string(), Logo { svg, tint }))
        })
        .collect()
}

/// 整份换掉缓存里的：先写进点开头的暂存目录，再换上，最后写 `fetched`。
fn store(dir: &Path, logos: &BTreeMap<String, Logo>) -> Result<(), String> {
    let parent = dir.parent().ok_or("the logo directory has no parent")?;
    let staged = parent.join(".logos.new");
    let failed = |error: std::io::Error| format!("logos not stored: {error}");
    if staged.exists() {
        std::fs::remove_dir_all(&staged).map_err(failed)?;
    }
    std::fs::create_dir_all(&staged).map_err(failed)?;
    for (id, logo) in logos {
        let name = match logo.tint {
            true => format!("{id}.svg"),
            false => format!("{id}.color.svg"),
        };
        std::fs::write(staged.join(name), &logo.svg).map_err(failed)?;
    }
    let now = Timestamp::from_unix_millis(super::refresh::now_millis())
        .map(|at| at.to_string())
        .ok_or("the clock is out of range")?;
    std::fs::write(staged.join(FETCHED), now).map_err(failed)?;
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(failed)?;
    }
    std::fs::rename(&staged, dir).map_err(failed)
}

/// 上次拉完的时刻；没有、读不懂的当没拉过。
fn fetched(dir: &Path) -> Option<SystemTime> {
    let text = std::fs::read_to_string(dir.join(FETCHED)).ok()?;
    let millis = u64::try_from(Timestamp::parse(text.trim()).ok()?.unix_millis()).ok()?;
    Some(SystemTime::UNIX_EPOCH + Duration::from_millis(millis))
}

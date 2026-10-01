//! 分块上传（`docs/blueprint/web-module.md`「六、分块上传」，施工 W-5）：`blob.open` 开一个上传、`blob.write`
//! 一块一块写、`blob.close` 收齐了存好，回应和 `blob.put` 一样（`attach.rs`）。
//!
//! [`Uploads`] 跟着连接走：住在这个连接的读循环里（`connection.rs`），编号只认开它的那个连接，别的连接拿来用
//! 回 `upload_unknown`。一个连接上的请求本来就一条条办（`protocol.md`「一个连接」第 1 条），不用锁。连接断了
//! （[`Uploads::discard_all`]）、60 秒没有 `blob.write`（[`Uploads::sweep`]）都作废，删掉暂存文件；核心起来时
//! 再清一遍崩了留下的（`crates/miyu-core/src/packages.rs`）。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::{ContentHash, FileName, Hasher, MediaType};
use miyu_store::blob::Blobs;

use crate::Core;
use crate::attach::{self, kind::kind};
use crate::refusal::Refusal;

/// 运行日志的来源。
const TARGET: &str = "miyu::endpoint";

/// 这个连接上同时最多开几个上传：第 5 个 `too_many_uploads`（`web-module.md`「怎么走」第六条第 1 款）。
const MAX_OPEN: usize = 4;

/// 一块最多多少字节：512 KiB（`web-module.md`「每个方法的参数和回应」）。
const CHUNK: usize = 512 * 1024;

/// `blob.open` 的参数：`name`、`media_type` 照 `blob.put` 第 1 条的写法查，`size` 必写。
#[derive(Debug, Deserialize)]
pub(crate) struct OpenParams {
    name: String,
    #[serde(default)]
    media_type: Option<String>,
    size: u64,
}

/// `blob.write` 的参数：`offset` 要正好等于已经收到的字节数。
#[derive(Debug, Deserialize)]
pub(crate) struct WriteParams {
    upload: String,
    offset: u64,
    data: String,
}

/// `blob.close` 的参数。
#[derive(Debug, Deserialize)]
pub(crate) struct CloseParams {
    upload: String,
}

/// 这个连接上开着的上传：编号 → 状态（施工 W-5）。
pub(crate) struct Uploads {
    /// 多久没写就作废。
    idle: Duration,
    open: BTreeMap<String, Entry>,
}

/// 一个开着的上传。
struct Entry {
    /// 暂存文件在哪：`blobs/tmp/upload-<编号>`。
    path: PathBuf,
    name: FileName,
    media_type: Option<MediaType>,
    /// `blob.open` 报的总共几个字节。
    size: u64,
    /// 已经收到几个字节：下一块要从这里接着写。
    received: u64,
    /// 边写边算的内容哈希（`web-module.md`「怎么走」第六条第 2 款）。
    hasher: Hasher,
    /// 上一次 `blob.open`、`blob.write` 是什么时候：超过 `idle` 没再写就作废。
    touched: Instant,
}

/// 这个连接上查 `upload` 的结果。
enum Claim {
    /// 还在、没过期：暂存文件在哪，已经收到几个，总共要收几个。
    Live(PathBuf, u64, u64),
    /// 刚发现过期了，已经从表里摘掉：暂存文件还在磁盘上，调用方删。
    Expired(PathBuf),
    /// 没有这个上传：编号不对、作废了、不是这个连接开的。
    Missing,
}

impl Uploads {
    /// 一张空表：这个连接还没开过上传。
    pub(crate) fn new(idle: Duration) -> Uploads {
        Uploads {
            idle,
            open: BTreeMap::new(),
        }
    }

    /// 清掉超过 `idle` 没再写的：从表里摘掉，交回它们的暂存文件（调用方删，这里不碰磁盘）。
    fn sweep(&mut self) -> Vec<PathBuf> {
        let idle = self.idle;
        let mut gone = Vec::new();
        self.open.retain(|_, entry| {
            let alive = entry.touched.elapsed() < idle;
            if !alive {
                gone.push(entry.path.clone());
            }
            alive
        });
        gone
    }

    /// 查 `upload`：过期的顺手从表里摘掉。
    fn claim(&mut self, upload: &str) -> Claim {
        let idle = self.idle;
        let expired =
            matches!(self.open.get(upload), Some(entry) if entry.touched.elapsed() >= idle);
        if expired {
            let path = self.open.remove(upload).expect("刚看到它还在").path;
            return Claim::Expired(path);
        }
        match self.open.get(upload) {
            Some(entry) => Claim::Live(entry.path.clone(), entry.received, entry.size),
            None => Claim::Missing,
        }
    }

    /// 连接要断了：这个连接开着的上传全部作废，删暂存文件（`web-module.md`「怎么走」第六条第 4 款）。
    pub(crate) async fn discard_all(&mut self, core: &Core) {
        let paths: Vec<PathBuf> = std::mem::take(&mut self.open)
            .into_values()
            .map(|entry| entry.path)
            .collect();
        if paths.is_empty() {
            return;
        }
        let blobs = blobs_of(core);
        let outcome = tokio::task::spawn_blocking(move || {
            for path in paths {
                blobs.discard_upload(&path);
            }
        })
        .await;
        if outcome.is_err() {
            tracing::error!(target: TARGET, "upload cleanup panicked");
        }
    }
}

/// `blob.open`：建一个新的暂存文件，交回上传编号。
pub(crate) async fn open(
    core: &Core,
    uploads: &mut Uploads,
    params: OpenParams,
) -> Result<Value, Refusal> {
    let name = attach::file_name(&params.name)?;
    let media_type = params
        .media_type
        .as_deref()
        .map(attach::media_type)
        .transpose()?;
    if params.size > attach::LIMIT {
        return Err(Refusal::ATTACHMENT_TOO_BIG);
    }
    for path in uploads.sweep() {
        discard(core, path).await;
    }
    if uploads.open.len() >= MAX_OPEN {
        return Err(Refusal::TOO_MANY_UPLOADS);
    }
    let id = upload_id()?;
    let blobs = blobs_of(core);
    let created = id.clone();
    let path = blocking(move || {
        blobs.create_upload(&created).map_err(|error| {
            tracing::warn!(target: TARGET, error = %error, "upload not stored");
            Refusal::INTERNAL
        })
    })
    .await?;
    uploads.open.insert(
        id.clone(),
        Entry {
            path,
            name,
            media_type,
            size: params.size,
            received: 0,
            hasher: Hasher::default(),
            touched: Instant::now(),
        },
    );
    Ok(json!({"upload": id}))
}

/// `blob.write`：写一块进暂存文件，交回一共收到几个字节。
pub(crate) async fn write(
    core: &Core,
    uploads: &mut Uploads,
    params: WriteParams,
) -> Result<Value, Refusal> {
    let data = STANDARD
        .decode(&params.data)
        .map_err(|_| Refusal::BAD_PARAMS)?;
    if data.len() > CHUNK {
        return Err(Refusal::BAD_PARAMS);
    }
    let (path, received, size) = match uploads.claim(&params.upload) {
        Claim::Live(path, received, size) => (path, received, size),
        Claim::Expired(path) => {
            discard(core, path).await;
            return Err(Refusal::UPLOAD_UNKNOWN);
        }
        Claim::Missing => return Err(Refusal::UPLOAD_UNKNOWN),
    };
    if params.offset != received {
        return Err(Refusal::upload_offset(received));
    }
    let added = data.len() as u64;
    if received.saturating_add(added) > size {
        return Err(Refusal::BAD_PARAMS);
    }
    let blobs = blobs_of(core);
    let chunk_path = path.clone();
    let chunk_data = data.clone();
    let offset = params.offset;
    blocking(move || {
        blobs
            .write_upload_chunk(&chunk_path, offset, &chunk_data)
            .map_err(|error| {
                tracing::warn!(target: TARGET, error = %error, "upload not stored");
                Refusal::INTERNAL
            })
    })
    .await?;
    let entry = uploads
        .open
        .get_mut(&params.upload)
        .expect("刚确认过还在，写之间这个连接上没有别的请求能动它");
    entry.hasher.update(&data);
    entry.received += added;
    entry.touched = Instant::now();
    Ok(json!({"received": entry.received}))
}

/// `blob.close`：收齐了认是什么、存成 blob，回应和 `blob.put` 一样。
pub(crate) async fn close(
    core: &Core,
    uploads: &mut Uploads,
    params: CloseParams,
) -> Result<Value, Refusal> {
    match uploads.claim(&params.upload) {
        Claim::Missing => Err(Refusal::UPLOAD_UNKNOWN),
        Claim::Expired(path) => {
            discard(core, path).await;
            Err(Refusal::UPLOAD_UNKNOWN)
        }
        Claim::Live(path, received, size) => {
            if received < size {
                return Err(Refusal::upload_incomplete(received));
            }
            let entry = uploads
                .open
                .remove(&params.upload)
                .expect("claim 刚确认过在");
            let hash = entry.hasher.finish();
            let blobs = blobs_of(core);
            blocking(move || finish_blocking(&blobs, &path, &hash, &entry.name, entry.media_type))
                .await
        }
    }
}

/// 在阻塞线程里：认是什么、照 [`Blobs::finish_upload`] 改名进位置；任何一步没成，暂存文件扔掉（和 `blob.put`
/// 不一样，暂存文件已经落在磁盘上了，不清就一直留着）。
fn finish_blocking(
    blobs: &Blobs,
    path: &std::path::Path,
    hash: &ContentHash,
    name: &FileName,
    media_type: Option<MediaType>,
) -> Result<Value, Refusal> {
    let outcome = finish_inner(blobs, path, hash, name, media_type);
    if outcome.is_err() {
        blobs.discard_upload(path);
    }
    outcome
}

fn finish_inner(
    blobs: &Blobs,
    path: &std::path::Path,
    hash: &ContentHash,
    name: &FileName,
    media_type: Option<MediaType>,
) -> Result<Value, Refusal> {
    let bytes = std::fs::read(path).map_err(|error| {
        tracing::warn!(target: TARGET, error = %error, "upload not stored");
        Refusal::INTERNAL
    })?;
    let found = kind(&bytes, media_type).map_err(|_| Refusal::ATTACHMENT_TOO_BIG)?;
    blobs.finish_upload(path, hash).map_err(|error| {
        tracing::warn!(target: TARGET, error = %error, "upload not stored");
        Refusal::INTERNAL
    })?;
    Ok(attach::reply(hash, name, found))
}

/// 作废一个上传：删它的暂存文件（删不掉留给核心起来时再清，[`Blobs::clear_uploads`]）。
async fn discard(core: &Core, path: PathBuf) {
    let blobs = blobs_of(core);
    let outcome = tokio::task::spawn_blocking(move || blobs.discard_upload(&path)).await;
    if outcome.is_err() {
        tracing::error!(target: TARGET, "upload cleanup panicked");
    }
}

/// 这个账号的 blob 存取。
fn blobs_of(core: &Core) -> Blobs {
    Blobs::new(core.root.blobs(&core.admin))
}

/// 一个新的上传编号：16 位小写十六进制，8 个随机字节（`web-module.md`「怎么走」第六条第 1 款）。
///
/// # Errors
///
/// 取不到随机数：这台机器的随机数源坏了，算核心自己的问题。
fn upload_id() -> Result<String, Refusal> {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).map_err(|error| {
        tracing::warn!(target: TARGET, error = %error, "upload not stored");
        Refusal::INTERNAL
    })?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// 在阻塞线程里跑；崩了的是核心的问题。
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Refusal> + Send + 'static,
) -> Result<T, Refusal> {
    tokio::task::spawn_blocking(work)
        .await
        .unwrap_or_else(|error| {
            tracing::error!(target: TARGET, error = %error, "upload panicked");
            Err(Refusal::INTERNAL)
        })
}

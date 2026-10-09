//! 把模型的文件备齐（施工 R-5 中，`docs/blueprint/recall.md` 第四条第 3 款）：放在缓存目录的 `embed/<id>/` 下。
//!
//! 1. 目录里清单以外的（崩了留下的临时文件、换下来的旧文件）删掉；
//! 2. 已经有的照清单的大小、SHA-256 核对，对不上的删掉（记一行 `WARN embedding model mismatch`）；
//! 3. 没有的下：边下边写临时文件、边算 SHA-256（[`miyu_store::generated::Staged`]），超过清单的大小当场停；大小、SHA-256
//!    都对才改名成正式的，对不上的不留。一个不成就停，交回原话（备的那一方记 `WARN embedding model not downloaded`）。
//!
//! 碰磁盘的核对、删、改名在阻塞线程里；下载那一路边收边写，每一块只是写进页缓存，落盘（`sync_data`）在提交时、阻塞线程里。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use miyu_http::{Client, Get, download};
use miyu_recall::embedding::{Manifest, ModelFile};
use miyu_store::generated::Staged;
use sha2::{Digest, Sha256};

use crate::TARGET;
use crate::blocking::blocking;

/// 下一个文件最多多久（从发出到读完）：24 MB 在慢的线路上也够。
const DOWNLOAD: Duration = Duration::from_secs(600);

/// 照清单 `manifest` 把文件备齐在 `dir` 里。
///
/// # Errors
///
/// 建不了目录、读不了盘；下不下来、下下来的大小或 SHA-256 对不上：英文的一句原话，带着文件名。
pub(super) async fn prepare(
    manifest: &Manifest,
    dir: &Path,
    client: &Client,
) -> Result<(), String> {
    let missing = {
        let (manifest, dir) = (manifest.clone(), dir.to_path_buf());
        blocking(move || check(&manifest, &dir)).await?
    };
    for file in missing {
        fetch(&manifest.id, &file, &dir.join(&file.name), client).await?;
    }
    Ok(())
}

/// 建目录、删清单以外的、核对已经有的：交回要下的那几个。
fn check(manifest: &Manifest, dir: &Path) -> Result<Vec<ModelFile>, String> {
    let failed = |error: std::io::Error| format!("{}: {error}", dir.display());
    std::fs::create_dir_all(dir).map_err(failed)?;
    for entry in std::fs::read_dir(dir).map_err(failed)? {
        let path = entry.map_err(failed)?.path();
        let listed = path
            .file_name()
            .is_some_and(|name| manifest.files.iter().any(|file| name == file.name.as_str()));
        if !listed {
            remove(&path)?;
        }
    }
    let mut missing = Vec::new();
    for file in &manifest.files {
        let path = dir.join(&file.name);
        if !path.exists() {
            missing.push(file.clone());
            continue;
        }
        if !matches(&path, file)? {
            tracing::warn!(target: TARGET, file = file.name.as_str(), "embedding model mismatch");
            remove(&path)?;
            missing.push(file.clone());
        }
    }
    Ok(missing)
}

/// 删掉 `path`（文件或目录）。
fn remove(path: &Path) -> Result<(), String> {
    let removed = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    removed.map_err(|error| format!("{}: {error}", path.display()))
}

/// `path` 的大小、SHA-256 和清单的 `file` 对不对得上。
fn matches(path: &Path, file: &ModelFile) -> Result<bool, String> {
    let failed = |error: std::io::Error| format!("{}: {error}", path.display());
    // 先比大小：不一样的不用再读 24 MB 算一遍。
    if std::fs::metadata(path).map_err(failed)?.len() != file.size {
        return Ok(false);
    }
    let mut hasher = Sha256::new();
    let mut reader = std::fs::File::open(path).map_err(failed)?;
    let mut buffer = vec![0; 1 << 16];
    loop {
        let read = std::io::Read::read(&mut reader, &mut buffer).map_err(failed)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()) == file.sha256)
}

/// 下 `file` 到 `path`：边下边写临时文件、边算，对上了才改名。
async fn fetch(model: &str, file: &ModelFile, path: &Path, client: &Client) -> Result<(), String> {
    let started = Instant::now();
    let named = |why: String| format!("{}: {why}", file.name);
    let mut staged = Staged::create(path).map_err(|error| named(error.to_string()))?;
    let mut hasher = Sha256::new();
    let limit = usize::try_from(file.size).unwrap_or(usize::MAX);
    let get = Get {
        client,
        url: &file.url,
        headers: &[],
        etag: None,
        timeout: DOWNLOAD,
        limit,
    };
    let mut each = |chunk: &[u8]| {
        hasher.update(chunk);
        staged.write(chunk)
    };
    let bytes = download(get, &mut each).await.map_err(named)?;
    // 大小不另比：多了的下的时候就停了（`limit`），少了的 SHA-256 一样对不上。
    if hex(&hasher.finalize()) != file.sha256 {
        return Err(named(format!("SHA-256 does not match ({bytes} bytes)")));
    }
    blocking(move || staged.commit())
        .await
        .map_err(|error| named(error.to_string()))?;
    tracing::info!(
        target: TARGET,
        model,
        file = file.name.as_str(),
        bytes,
        took_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "embedding model downloaded"
    );
    Ok(())
}

/// 十六进制的小写写法（清单里 SHA-256 的写法）。
fn hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 模型的文件放在缓存目录 `cache`（缓存目录下的 `embed`）的哪一格：`<id>/`。
pub(super) fn dir_of(cache: &Path, manifest: &Manifest) -> PathBuf {
    cache.join(&manifest.id)
}

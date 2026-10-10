//! 核对包里的模型文件（施工 R-5 三补，`docs/blueprint/recall.md` 第四条第 3 款）：内置模型这个小程序包把模型文件放在包目录里，
//! 装好就能用、不下（施工 R-5 中做的下载随这一步去掉）。第一次要向量时在后台照模型清单的大小、SHA-256 一个个核对：少了的、
//! 对不上的这一回用不了（记一行 `WARN embedding model mismatch`），不删包里的东西：那是装的那一方的。
//!
//! 读盘在阻塞线程里，调的一方管。

use std::path::Path;

use miyu_recall::embedding::{Manifest, ModelFile};
use sha2::{Digest, Sha256};

use crate::TARGET;

/// 照模型清单 `manifest` 核对 `dir` 里的文件。
///
/// # Errors
///
/// 少了一个、读不了、大小或 SHA-256 对不上：英文的一句原话，带着文件名。
pub(super) fn check(manifest: &Manifest, dir: &Path) -> Result<(), String> {
    for file in &manifest.files {
        let path = dir.join(&file.name);
        if !path.is_file() {
            return Err(format!("{}: missing from {}", file.name, dir.display()));
        }
        if !matches(&path, file)? {
            tracing::warn!(target: TARGET, file = file.name.as_str(), "embedding model mismatch");
            return Err(format!("{}: size or SHA-256 does not match", file.name));
        }
    }
    Ok(())
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

/// 十六进制的小写写法（清单里 SHA-256 的写法）。
fn hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

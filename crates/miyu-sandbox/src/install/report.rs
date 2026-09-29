//! 提升过的自己没成时写下的原因（`docs/blueprint/sandbox/windows.md`「对外的样子」、「怎么走」第 3、6 条）：本人数据根里的
//! `state/sandbox/windows-error.json`，一行 JSON，就是 [`InstallError`] 照原样写出来。
//!
//! 提升过的自己的窗口是藏起来的，它说的话人看不到；等它的那一个读了这份、照界面语言说给人听、删掉。起它之前先删掉
//! 上次留下的，免得把旧的原因当成这一次的。

use super::{InstallError, Owner, files};

/// 原因的文件名，在数据根的 `state/sandbox/` 里。
pub(crate) const FILE: &str = "windows-error.json";

/// 提升过的自己写下没成的原因：认标记、不经链接、只给本人和 SYSTEM，盖掉旧的。
///
/// # Errors
///
/// 数据根没有标记；写不了（`write report`）。
pub fn report(owner: &Owner, error: &InstallError) -> Result<(), InstallError> {
    files::check_root(owner.home())?;
    let failed = |detail: &dyn std::fmt::Display| InstallError::failed("write report", detail);
    let line = serde_json::to_string(error).map_err(|error| failed(&error))?;
    files::replace_private(
        owner.home(),
        FILE,
        format!("{line}\n").as_bytes(),
        owner.sid(),
    )
    .map_err(|error| failed(&error))
}

/// 等它的那一个读回原因：没有的是 `None`。
///
/// # Errors
///
/// 读不了、读不懂（`read report`）。
pub fn reported(owner: &Owner) -> Result<Option<InstallError>, InstallError> {
    let failed = |detail: &dyn std::fmt::Display| InstallError::failed("read report", detail);
    let text = match std::fs::read_to_string(path(owner)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(failed(&error)),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| failed(&error))
}

/// 删掉留下的原因，本来没有不算错。
///
/// # Errors
///
/// 删不了（`clear report`）。
pub fn clear_report(owner: &Owner) -> Result<(), InstallError> {
    files::remove_if_there(&path(owner))
        .map_err(|error| InstallError::failed("clear report", error))
}

/// 原因在哪。
fn path(owner: &Owner) -> std::path::PathBuf {
    owner.home().join("state").join("sandbox").join(FILE)
}

#[cfg(test)]
mod tests;

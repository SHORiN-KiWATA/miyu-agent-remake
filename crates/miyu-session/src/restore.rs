//! 改回文件（`docs/designs/10-自带软件.md` 第七节「改回文件的细则」，施工 4-7 上）：照内核给的几步，一步一步先核对
//! 原处是不是该有的样子，对上了才动手；一步一项交回结局，内核记成 `files.restored`。碰磁盘，在阻塞线程里调。
//!
//! 写回用 [`miyu_fs::replace()`]，进出回收站用 [`miyu_fs::trash`]：和写的工具用的是同一份。

use std::fs;
use std::io;
use std::path::Path;

use miyu_fs::trash::{self, Refused};
use miyu_kernel::event::{RestoreOutcome, Restored};
use miyu_kernel::id::ContentHash;
use miyu_kernel::session::{Expect, Step, StepAction};
use miyu_store::blob::{BlobError, Blobs};

/// 照先后做完这几步。要写回的内容从 `blobs` 里取，`home` 是交给回收站的家目录（Linux 的家目录回收站照它找）。
pub(crate) fn restore(steps: &[Step], blobs: &Blobs, home: Option<&Path>) -> Vec<Restored> {
    steps.iter().map(|step| one(step, blobs, home)).collect()
}

/// 做成了：改回了；移进了回收站的这个位置；移回来了，是文件的附上它的哈希。
enum Done {
    Restored,
    Trashed(String),
    Back(Option<ContentHash>),
}

/// 没做成：对不上的哪一种（内容被改过的附上现在的哈希），或者出错了。
enum Missed {
    Changed(Option<ContentHash>),
    Outcome(RestoreOutcome),
    Failed(io::Error),
}

impl From<io::Error> for Missed {
    fn from(error: io::Error) -> Missed {
        Missed::Failed(error)
    }
}

/// 原处现在是什么样。
enum Now {
    /// 什么都没有。
    Absent,
    /// 一个文件，内容的哈希。
    File(ContentHash),
    /// 别的：目录、链接；不读内容时的有东西（[`there`]）。
    Other,
}

/// 做一步，交回它的结局。
fn one(step: &Step, blobs: &Blobs, home: Option<&Path>) -> Restored {
    let path = Path::new(&step.path);
    let result = match &step.action {
        StepAction::Write { expect, content } => write(path, expect, content, blobs),
        StepAction::Trash { expect } => put(path, expect, home),
        StepAction::Untrash { from } => untrash(path, Path::new(from)),
    };
    let mut restored = step.restored();
    match result {
        Ok(Done::Restored) => {}
        Ok(Done::Trashed(trash)) => restored.trash = Some(trash),
        Ok(Done::Back(hash)) => restored.hash = hash,
        Err(Missed::Changed(found)) => {
            restored.outcome = RestoreOutcome::Changed;
            restored.found = found;
        }
        Err(Missed::Outcome(outcome)) => restored.outcome = outcome,
        Err(Missed::Failed(error)) => {
            restored.outcome = RestoreOutcome::Failed;
            restored.error = Some(error.to_string());
        }
    }
    restored
}

/// 写回 `content`：现在已经是它的，算改回了；原处要是 `expect` 的样子；上级目录没了的建上。
fn write(
    path: &Path,
    expect: &Expect,
    content: &ContentHash,
    blobs: &Blobs,
) -> Result<Done, Missed> {
    let now = now(path)?;
    if matches!(&now, Now::File(hash) if hash == content) {
        return Ok(Done::Restored);
    }
    check(expect, &now)?;
    let bytes = blobs.get(content).map_err(|error| match error {
        BlobError::Missing(_) | BlobError::Corrupt(_) => Missed::Outcome(RestoreOutcome::Unsaved),
        BlobError::Io(error) => Missed::Failed(error),
    })?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    miyu_fs::replace(path, &bytes)?;
    Ok(Done::Restored)
}

/// 移进回收站：原处要是 `expect` 的样子。回收站收不了的不删；挪了却找不到的，当回收站里没有它。只要有东西就行的
/// （`Expect::Present`），不读内容：读不了的文件照样移（施工 4-9 再补一）。
fn put(path: &Path, expect: &Expect, home: Option<&Path>) -> Result<Done, Missed> {
    let now = match expect {
        Expect::Present => there(path)?,
        Expect::Absent | Expect::Content(_) => now(path)?,
    };
    if matches!(now, Now::Absent) {
        return Err(Missed::Outcome(RestoreOutcome::Missing));
    }
    check(expect, &now)?;
    match trash::put(path, home) {
        Ok(location) => Ok(Done::Trashed(location)),
        Err(Refused::Unavailable) => Err(Missed::Outcome(RestoreOutcome::Unavailable)),
        Err(Refused::Lost) => Err(Missed::Outcome(RestoreOutcome::Gone)),
        Err(Refused::Failed(error)) => Err(Missed::Failed(error)),
    }
}

/// 从回收站的 `from` 移回 `path`：原处要空着，回收站里要还在。移回来的是文件的，记下它的哈希。
fn untrash(path: &Path, from: &Path) -> Result<Done, Missed> {
    if !matches!(now(path)?, Now::Absent) {
        return Err(Missed::Outcome(RestoreOutcome::Occupied));
    }
    match fs::symlink_metadata(from) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(Missed::Outcome(RestoreOutcome::Gone));
        }
        Err(error) => return Err(Missed::Failed(error)),
        Ok(_) => {}
    }
    trash::restore(from, path)?;
    // 移回来了才看哈希：看不了的（例如读不了）照样算移回来了，只是不附哈希（施工 4-9 再补一）。记成失败的话，
    // 下一次恢复会当它还在回收站的旧位置。
    Ok(Done::Back(match now(path) {
        Ok(Now::File(hash)) => Some(hash),
        Ok(Now::Absent | Now::Other) | Err(_) => None,
    }))
}

/// 原处是不是 `expect` 的样子。
fn check(expect: &Expect, now: &Now) -> Result<(), Missed> {
    match (expect, now) {
        (Expect::Absent, Now::Absent) => Ok(()),
        (Expect::Absent, _) => Err(Missed::Outcome(RestoreOutcome::Occupied)),
        (Expect::Content(_) | Expect::Present, Now::Absent) => {
            Err(Missed::Outcome(RestoreOutcome::Missing))
        }
        (Expect::Content(want), Now::File(hash)) if want == hash => Ok(()),
        (Expect::Content(_), Now::File(hash)) => Err(Missed::Changed(Some(hash.clone()))),
        (Expect::Content(_), Now::Other) => Err(Missed::Changed(None)),
        (Expect::Present, _) => Ok(()),
    }
}

/// 原处现在是什么样：不跟最后一层的链接，文件读出来算哈希。
fn now(path: &Path) -> io::Result<Now> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Now::Absent),
        Err(error) => Err(error),
        Ok(meta) if meta.is_file() => Ok(Now::File(ContentHash::of(&fs::read(path)?))),
        Ok(_) => Ok(Now::Other),
    }
}

/// 原处有没有东西，不读内容：有的算「别的」。
fn there(path: &Path) -> io::Result<Now> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Now::Absent),
        Err(error) => Err(error),
        Ok(_) => Ok(Now::Other),
    }
}

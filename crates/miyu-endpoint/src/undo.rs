//! 撤销、恢复的回应里给人看的几样（`docs/designs/04-核心协议.md` 第九节，施工 4-7 下；重做的撤销那一半也照它写，施工 4-7
//! 再补）：撤的是哪一轮（那一轮人说的那句话）、撤掉的几轮执行过几条命令、撤掉了几次压缩（施工 6-9）、几次清空（施工 6-8
//! 补）、停掉了哪几个任务（施工 7-8，`jobs.rs`）、每个文件怎样、之后又被改过的差异。「该显示什么」写在核心里（`01-架构.md`
//! D1），头照着印；做视图投影（M8）时这几样挪进视图，字段只加不改。
//!
//! 照会话的日志读：撤销（恢复）和改回文件的结局都落了盘才回应，这时读得到。碰磁盘，在阻塞线程里调。
//!
//! 改回了的（`outcome` 是 `restored`、`action` 是 `write` 的）也带差异（施工 4-7 再补）：改回以前磁盘上该是什么样，
//! 对改回以后的。两边都是账本里的 blob，不读现在的磁盘——`restored` 时磁盘上已经是改回以后的样子了。`trash`、
//! `untrash` 只是挪位置，内容没变，不带。

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::{Value, json};
use similar::TextDiff;

use miyu_kernel::block::Block;
use miyu_kernel::event::{
    Body, CompactTrigger, Effect, Event, FileChanged, RestoreAction, RestoreOutcome, Restored,
    ToolResult, ToolStatus,
};
use miyu_kernel::id::{ContentHash, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::tool::Access;
use miyu_store::blob::Blobs;
use miyu_store::log::read_events;

use crate::Core;

mod jobs;

/// 一个文件的差异最多交几行。
const DIFF_LINES: usize = 20;
/// 任一边超过这么多字节的不算差异。
const DIFF_BYTES: u64 = 1 << 20;
/// 内核给跑到一半被打断的、重启时没跑完的调用记的那两句（`kernel/session.md`）：这两种都可能跑了一半。
const PARTLY_RAN: [&str; 2] = [
    "core/tool-results/cancelled-running",
    "core/tool-results/restarted",
];

/// 撤销、恢复被接受了：回应是它产生的事件的序号 `events`、会话的工作目录 `cwd`，和给人看的几样。
pub(crate) async fn reply(core: &Core, session: &SessionId, cwd: &str, events: Vec<u64>) -> Value {
    // 照属主的家目录读日志和 blob（施工 O-4 上）；回应时会话还在表里，找不到的照管理员（不会走到）。
    let owner = core
        .sessions
        .owner(core, session)
        .await
        .unwrap_or_else(|| core.admin.clone());
    let sources = Sources {
        log: core.root.session_dir(&owner, session),
        blobs: core.root.blobs(&owner),
        executing: core
            .tools()
            .specs()
            .filter(|spec| spec.access == Access::Execute)
            .map(|spec| spec.name.clone())
            .collect(),
    };
    let seqs = events.clone();
    let cwd = cwd.to_string();
    let written = tokio::task::spawn_blocking(move || {
        // 工作目录照真实的位置写：效果里的路径是真实的位置，头照它写相对的才对得上。
        let cwd =
            std::fs::canonicalize(&cwd).map_or(cwd, |real| real.to_string_lossy().into_owned());
        (report(&sources, &seqs), plain(&cwd))
    })
    .await;
    let (report, cwd) = match written {
        Ok(written) => written,
        Err(error) => {
            tracing::error!(target: "miyu::endpoint", error = %error, "undo report panicked");
            (Report::default(), String::new())
        }
    };
    let mut reply = serde_json::to_value(report).unwrap_or_else(|_| json!({}));
    reply["events"] = json!(events);
    reply["cwd"] = json!(cwd);
    reply
}

/// 给人看的路径：Windows 上真实的位置带着 `\\?\` 这个前缀，去掉；`\\?\UNC\` 的照原样。
fn plain(path: &str) -> String {
    match path.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => rest.to_string(),
        _ => path.to_string(),
    }
}

/// 写回应要照的：会话的日志、账号的 blob 在哪，哪几件工具是执行命令的。
pub(crate) struct Sources {
    pub(crate) log: PathBuf,
    pub(crate) blobs: PathBuf,
    pub(crate) executing: BTreeSet<String>,
}

/// 回应里给人看的几样。
#[derive(Debug, Default, Serialize)]
pub(crate) struct Report {
    /// 撤了（恢复了）几轮。
    turns: usize,
    /// 第一轮里人说的那句话的第一行；那一轮不是人开的，没有。
    #[serde(skip_serializing_if = "Option::is_none")]
    said: Option<String>,
    /// 撤掉的几轮执行过几条命令：撤销时才有。
    #[serde(skip_serializing_if = "Option::is_none")]
    commands: Option<usize>,
    /// 撤掉了几次压缩（施工 6-9）：撤销时才有，是 0 的不写。头照它说一句上下文回到了压缩前。清空不算在里面。
    #[serde(skip_serializing_if = "Option::is_none")]
    compactions: Option<usize>,
    /// 撤掉了几次清空（施工 6-8 补）：撤销时才有，是 0 的不写。头照它说一句上下文回到了清空以前。
    #[serde(skip_serializing_if = "Option::is_none")]
    clears: Option<usize>,
    /// 停掉的任务（施工 7-8）：撤掉的几轮派出去、撤销那一刻还在跑的，照编号。撤销时才有，没有的不写。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    jobs: Vec<jobs::Stopped>,
    /// 改回的每一步，照先后。
    files: Vec<File>,
}

/// 改回的一步。
#[derive(Debug, Serialize)]
struct File {
    path: String,
    action: String,
    outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    /// 有差异的（之后又被改过的 `changed`，或者改回了内容的 `restored`）。
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    diff: Option<Diff>,
}

/// 一份差异：统一格式，上下文 3 行，最多 [`DIFF_LINES`] 行，不带 `---`、`+++` 那两行；外加新增、删掉了几行，照
/// 整份差异数，不照截断以后的。
#[derive(Debug, Serialize)]
struct Diff {
    diff: Vec<String>,
    /// 差异里没交出来的行数。
    #[serde(skip_serializing_if = "is_zero")]
    more: usize,
    added: usize,
    removed: usize,
}

/// 照撤销（恢复）那一条和改回文件的结局那一条（`events` 里头两个序号）写成回应里的几样。日志读不出来的，
/// 记一条运行日志，交回空的：撤销本身已经成了。
pub(crate) fn report(sources: &Sources, events: &[u64]) -> Report {
    let log = match read_events(&sources.log) {
        Ok(log) => log,
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "undo report not written");
            return Report::default();
        }
    };
    let find = |seq: u64| log.iter().find(|event| event.seq.get() == seq);
    let Some(first) = events.first().and_then(|seq| find(*seq)) else {
        return Report::default();
    };
    let (turns, undo) = match &first.body {
        Body::TurnReverted(reverted) => (reverted.turns.clone(), true),
        Body::TurnUnreverted(unreverted) => (unreverted.turns.clone(), false),
        _ => return Report::default(),
    };
    let restored = events
        .get(1)
        .and_then(|seq| find(*seq))
        .and_then(|event| match &event.body {
            Body::FilesRestored(restored) => Some(restored.files.as_slice()),
            _ => None,
        })
        .unwrap_or_default();
    let blobs = Blobs::new(sources.blobs.clone());
    Report {
        turns: turns.len(),
        said: turns.first().and_then(|turn| said(&log, *turn)),
        commands: undo.then(|| commands(&log, &turns, &sources.executing)),
        compactions: undo
            .then(|| compactions(&log, &turns, false))
            .filter(|count| *count > 0),
        clears: undo
            .then(|| compactions(&log, &turns, true))
            .filter(|count| *count > 0),
        jobs: match undo {
            true => jobs::stopped(&log, first.seq, &turns),
            false => Vec::new(),
        },
        files: restored
            .iter()
            .map(|file| entry(file, &log, &blobs, undo))
            .collect(),
    }
}

/// 回合 `turn` 里人说的那句话的第一行：照 `turn.started` 找引起它的那一条，是人亲口说的才算。没有触发的（手动压缩、
/// 清空单开的那一轮，施工 6-8、6-8 补）没有。
fn said(log: &[Event], turn: TurnId) -> Option<String> {
    let trigger = log.iter().find_map(|event| match &event.body {
        Body::TurnStarted(started) if event.seq == turn.started() => started.trigger,
        _ => None,
    })?;
    let event = log.iter().find(|event| event.seq == trigger)?;
    let (Body::MessageUser(message), By::Person(_)) = (&event.body, &event.by) else {
        return None;
    };
    message.blocks.iter().find_map(|block| match block {
        Block::Text(text) => text
            .text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(str::to_string),
        _ => None,
    })
}

/// 这几轮里有几次压缩（施工 6-9）：`context.compacted` 带的回合在这几轮里的。`clears` 是数清空的（`trigger` 是
/// `clear`，施工 6-8 补），不是的数别的几种。
fn compactions(log: &[Event], turns: &[TurnId], clears: bool) -> usize {
    log.iter()
        .filter(|event| event.turn.is_some_and(|turn| turns.contains(&turn)))
        .filter(|event| {
            matches!(&event.body, Body::ContextCompacted(compacted)
            if (compacted.trigger == Some(CompactTrigger::Clear)) == clears)
        })
        .count()
}

/// 这几轮里真跑过几条命令：回复里调的、照工具目录是执行命令的那几件，结果是成了、出错、可能跑了一半的；
/// 被拒的、没跑过的、跳过的不算（施工 4-9 再补一）。
fn commands(log: &[Event], turns: &[TurnId], executing: &BTreeSet<String>) -> usize {
    let within = |event: &&Event| event.turn.is_some_and(|turn| turns.contains(&turn));
    let ran: BTreeSet<_> = log
        .iter()
        .filter(within)
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) if ran_at_all(result) => Some(result.call_id),
            _ => None,
        })
        .collect();
    log.iter()
        .filter(within)
        .filter_map(|event| match &event.body {
            Body::MessageAssistant(reply) => Some(&reply.blocks),
            _ => None,
        })
        .flatten()
        .filter(|block| {
            matches!(block, Block::ToolCall(call)
                if executing.contains(&call.name) && ran.contains(&call.call_id))
        })
        .count()
}

/// 这个结果说明调用跑过：成了、出错，或者可能跑了一半（跑到一半被打断、重启时没跑完）。
fn ran_at_all(result: &ToolResult) -> bool {
    match result.status {
        ToolStatus::Ok | ToolStatus::Error => true,
        ToolStatus::Cancelled => result
            .human
            .as_ref()
            .is_some_and(|said| PARTLY_RAN.contains(&said.key.as_str())),
        _ => false,
    }
}

/// 改回的一步写成回应里的一项：之后又被改过的（`changed`）附上她改完的对现在的差异；改回了内容的（`restored`、
/// `action` 是 `write`）附上改回以前对改回以后的差异。`trash`、`untrash` 只是挪位置，内容没变，不附；`restored`
/// 里本来就一样、没写的，两边内容相同，差异算出来是空的，`diff` 自然是 `None`，也不附。
fn entry(file: &Restored, log: &[Event], blobs: &Blobs, undo: bool) -> File {
    let diff = if matches!(file.outcome, RestoreOutcome::Changed) {
        expected(file, log, undo).and_then(|expected| changed_diff(blobs, &expected, &file.path))
    } else if matches!(file.outcome, RestoreOutcome::Restored)
        && matches!(file.action, RestoreAction::Write)
    {
        restored_sides(file, log, undo)
            .and_then(|(before, after)| restored_diff(blobs, before.as_ref(), after.as_ref()))
    } else {
        None
    };
    File {
        path: plain(&file.path),
        action: file.action.as_str().to_string(),
        outcome: file.outcome.as_str().to_string(),
        error: file.error.clone(),
        diff,
    }
}

/// 这一步照的那条 `tool.result` 的第几个效果：是 `file.changed` 的才有。
fn file_changed<'a>(file: &Restored, log: &'a [Event]) -> Option<&'a FileChanged> {
    let result = log.iter().find(|event| event.seq == file.result)?;
    let Body::ToolResult(result) = &result.body else {
        return None;
    };
    match result.effects.get(usize::try_from(file.effect).ok()?)? {
        Effect::FileChanged(changed) => Some(changed),
        _ => None,
    }
}

/// 这一步动手前原处该是什么样（`changed` 用）：撤销时是她改完的（`file.changed` 的改后），恢复时是撤销以后的
/// （改前）。不是 `file.changed` 效果的（`file.trashed`）没有。
fn expected(file: &Restored, log: &[Event], undo: bool) -> Option<ContentHash> {
    let changed = file_changed(file, log)?;
    if undo {
        Some(changed.after.clone())
    } else {
        changed.before.clone()
    }
}

/// `restored`、`action` 是 `write` 的这一步，改回以前、改回以后磁盘上该是什么样：撤销时前者是她改完的、后者是
/// 改前的；恢复时反过来。没有内容（新建的文件改回以前、后来被挪开再写回的文件改回以后）的是 `None`，当空的算。
/// 不是 `file.changed` 效果的，整个没有。
fn restored_sides(
    file: &Restored,
    log: &[Event],
    undo: bool,
) -> Option<(Option<ContentHash>, Option<ContentHash>)> {
    let changed = file_changed(file, log)?;
    Some(if undo {
        (Some(changed.after.clone()), changed.before.clone())
    } else {
        (changed.before.clone(), Some(changed.after.clone()))
    })
}

/// `expected`（blob）和现在的 `path`（磁盘）之间的差异：之后又被改过的（`changed`）用。
fn changed_diff(blobs: &Blobs, expected: &ContentHash, path: &str) -> Option<Diff> {
    let then = blob_bytes(blobs, Some(expected))?;
    let now = match std::fs::metadata(path) {
        Ok(meta) if meta.len() <= DIFF_BYTES => std::fs::read(path).ok(),
        _ => None,
    }?;
    diff(&then, &now)
}

/// `before`、`after`（都是 blob，`None` 当空的算）之间的差异：改回了内容的（`restored`）用，两边都不读现在的
/// 磁盘。
fn restored_diff(
    blobs: &Blobs,
    before: Option<&ContentHash>,
    after: Option<&ContentHash>,
) -> Option<Diff> {
    let then = blob_bytes(blobs, before)?;
    let now = blob_bytes(blobs, after)?;
    diff(&then, &now)
}

/// `hash` 的内容：没有 `hash`（`None`）的照空的算；超过 [`DIFF_BYTES`]、取不出来的没有。
fn blob_bytes(blobs: &Blobs, hash: Option<&ContentHash>) -> Option<Vec<u8>> {
    let bytes = match hash {
        None => Vec::new(),
        Some(hash) => blobs.get(hash).ok()?,
    };
    (u64::try_from(bytes.len()).unwrap_or(u64::MAX) <= DIFF_BYTES).then_some(bytes)
}

/// `then` 和 `now` 之间的差异：统一格式，上下文 3 行，最多 [`DIFF_LINES`] 行，外加整份差异数的新增、删掉的行数。
/// 不是文本的、两边一样（没有差异）的，没有。
fn diff(then: &[u8], now: &[u8]) -> Option<Diff> {
    let (Ok(then), Ok(now)) = (std::str::from_utf8(then), std::str::from_utf8(now)) else {
        return None;
    };
    let text = TextDiff::from_lines(then, now)
        .unified_diff()
        .context_radius(3)
        .missing_newline_hint(false)
        .to_string();
    if text.is_empty() {
        return None;
    }
    let lines: Vec<&str> = text.lines().collect();
    let added = lines.iter().filter(|line| line.starts_with('+')).count();
    let removed = lines.iter().filter(|line| line.starts_with('-')).count();
    let more = lines.len().saturating_sub(DIFF_LINES);
    let diff = lines
        .into_iter()
        .take(DIFF_LINES)
        .map(str::to_string)
        .collect();
    Some(Diff {
        diff,
        more,
        added,
        removed,
    })
}

/// 没有没交出来的行，不写 `more`。
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde 的 skip_serializing_if 只给引用"
)]
fn is_zero(n: &usize) -> bool {
    *n == 0
}

#[cfg(test)]
mod tests;

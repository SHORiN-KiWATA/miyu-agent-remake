//! 撤销、恢复的回应里给人看的几样（`docs/designs/04-核心协议.md` 第九节，施工 4-7 下）：撤的是哪一轮（那一轮人说的
//! 那句话）、撤掉的几轮执行过几条命令、撤掉了几次压缩（施工 6-9）、每个文件怎样、之后又被改过的差异。「该显示什么」写在核心里（`01-架构.md`
//! D1），头照着印；做视图投影（M8）时这几样挪进视图，字段只加不改。
//!
//! 照会话的日志读：撤销（恢复）和改回文件的结局都落了盘才回应，这时读得到。碰磁盘，在阻塞线程里调。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};
use similar::TextDiff;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Effect, Event, RestoreOutcome, Restored, ToolResult, ToolStatus};
use miyu_kernel::id::{ContentHash, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::tool::Access;
use miyu_store::blob::Blobs;
use miyu_store::log::read_events;

use crate::Core;

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
    let sources = Sources {
        log: core.root.session_dir(&core.admin, session),
        blobs: core.root.blobs(&core.admin),
        executing: core
            .tools
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
    /// 撤掉了几次压缩（施工 6-9）：撤销时才有，是 0 的不写。头照它说一句上下文回到了压缩前。
    #[serde(skip_serializing_if = "Option::is_none")]
    compactions: Option<usize>,
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
    /// 之后又被改过的：统一格式的差异，不带 `---`、`+++` 那两行，最多 [`DIFF_LINES`] 行。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    diff: Vec<String>,
    /// 差异里没交出来的行数。
    #[serde(skip_serializing_if = "is_zero")]
    more: usize,
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
    let (turns, undo) = match events
        .first()
        .and_then(|seq| find(*seq))
        .map(|event| &event.body)
    {
        Some(Body::TurnReverted(reverted)) => (reverted.turns.clone(), true),
        Some(Body::TurnUnreverted(unreverted)) => (unreverted.turns.clone(), false),
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
            .then(|| compactions(&log, &turns))
            .filter(|count| *count > 0),
        files: restored
            .iter()
            .map(|file| entry(file, &log, &blobs, undo))
            .collect(),
    }
}

/// 回合 `turn` 里人说的那句话的第一行：照 `turn.started` 找引起它的那一条，是人亲口说的才算。
fn said(log: &[Event], turn: TurnId) -> Option<String> {
    let trigger = log.iter().find_map(|event| match &event.body {
        Body::TurnStarted(started) if event.seq == turn.started() => Some(started.trigger),
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

/// 这几轮里有几次压缩（施工 6-9）：`context.compacted` 带的回合在这几轮里的。
fn compactions(log: &[Event], turns: &[TurnId]) -> usize {
    log.iter()
        .filter(|event| event.turn.is_some_and(|turn| turns.contains(&turn)))
        .filter(|event| matches!(event.body, Body::ContextCompacted(_)))
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

/// 改回的一步写成回应里的一项：之后又被改过的，附上差异。
fn entry(file: &Restored, log: &[Event], blobs: &Blobs, undo: bool) -> File {
    let (diff, more) = match file.outcome {
        RestoreOutcome::Changed => expected(file, log, undo)
            .map(|expected| diff(blobs, &expected, Path::new(&file.path)))
            .unwrap_or_default(),
        _ => (Vec::new(), 0),
    };
    File {
        path: plain(&file.path),
        action: file.action.as_str().to_string(),
        outcome: file.outcome.as_str().to_string(),
        error: file.error.clone(),
        diff,
        more,
    }
}

/// 这一步动手前原处该是什么样：撤销时是她改完的（`file.changed` 的改后），恢复时是撤销以后的（改前）。移回来
/// 的那种（`file.trashed`）没有存下内容，没有。
fn expected(file: &Restored, log: &[Event], undo: bool) -> Option<ContentHash> {
    let result = log.iter().find(|event| event.seq == file.result)?;
    let Body::ToolResult(result) = &result.body else {
        return None;
    };
    match result.effects.get(usize::try_from(file.effect).ok()?)? {
        Effect::FileChanged(changed) if undo => Some(changed.after.clone()),
        Effect::FileChanged(changed) => changed.before.clone(),
        _ => None,
    }
}

/// `expected`（blob）和现在的 `path` 之间的差异：统一格式，上下文 3 行，最多 [`DIFF_LINES`] 行，交回这几行和
/// 没交出来的行数。不是文本的、任一边太大的、取不出来的，没有。
fn diff(blobs: &Blobs, expected: &ContentHash, path: &Path) -> (Vec<String>, usize) {
    let now = match std::fs::metadata(path) {
        Ok(meta) if meta.len() <= DIFF_BYTES => std::fs::read(path).ok(),
        _ => None,
    };
    let Some(now) = now else {
        return (Vec::new(), 0);
    };
    let Ok(then) = blobs.get(expected) else {
        return (Vec::new(), 0);
    };
    let (Ok(then), Ok(now)) = (std::str::from_utf8(&then), std::str::from_utf8(&now)) else {
        return (Vec::new(), 0);
    };
    if u64::try_from(then.len()).unwrap_or(u64::MAX) > DIFF_BYTES {
        return (Vec::new(), 0);
    }
    let text = TextDiff::from_lines(then, now)
        .unified_diff()
        .context_radius(3)
        .missing_newline_hint(false)
        .to_string();
    let lines: Vec<&str> = text.lines().collect();
    let more = lines.len().saturating_sub(DIFF_LINES);
    let shown = lines
        .into_iter()
        .take(DIFF_LINES)
        .map(str::to_string)
        .collect();
    (shown, more)
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

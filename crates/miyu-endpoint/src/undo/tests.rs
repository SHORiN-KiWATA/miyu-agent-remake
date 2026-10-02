//! 撤销回应里「执行过几条命令」怎么认跑过的（施工 4-9 再补一）：成了、出错的算；可能跑了一半的（跑到一半被打断、
//! 重启时没跑完）算；被拒的、没跑过的、跳过的，和别的已取消不算。「撤掉了几次压缩」只数带着撤掉的回合的（施工 6-9），
//! 清空另数（施工 6-8 补）。
//!
//! 改回了内容的（`outcome` 是 `restored`、`action` 是 `write` 的）也带差异，连同新增、删掉的行数（施工 4-7 再补）：
//! 撤销时对照改回以前（她改完的）、改回以后（改前的）；恢复时反过来；没有的那一边（新建的文件改回以前）照空的算；
//! `trash`、`untrash`、两边一样（没写）的不带。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::event::{
    Body, CompactTrigger, ContextCompacted, Effect, Event, FileChanged, RestoreAction,
    RestoreOutcome, Restored, Said, ToolResult, ToolStatus,
};
use miyu_kernel::id::{CallId, ContentHash, Seq, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;
use miyu_store::blob::Blobs;

use super::{compactions, entry, ran_at_all};

/// 一条结果：状态是 `status`，内核写的那一句是 `key`。
fn result(status: ToolStatus, key: Option<&str>) -> ToolResult {
    ToolResult {
        call_id: Seq::new(6)
            .and_then(|seq| CallId::new(seq, 1))
            .expect("编号合写法"),
        status,
        blocks: Vec::new(),
        duration_ms: None,
        human: key.map(|key| Said::new(format!("core/tool-results/{key}"))),
        effects: Vec::new(),
    }
}

#[test]
fn what_ran_or_may_have_run_counts() {
    assert!(ran_at_all(&result(ToolStatus::Ok, None)));
    assert!(ran_at_all(&result(ToolStatus::Error, Some("crashed"))));
    assert!(ran_at_all(&result(
        ToolStatus::Cancelled,
        Some("cancelled-running")
    )));
    assert!(ran_at_all(&result(
        ToolStatus::Cancelled,
        Some("restarted")
    )));
}

#[test]
fn what_never_ran_does_not_count() {
    assert!(!ran_at_all(&result(
        ToolStatus::Cancelled,
        Some("cancelled-before")
    )));
    assert!(!ran_at_all(&result(ToolStatus::Cancelled, None)));
    assert!(!ran_at_all(&result(ToolStatus::Denied, Some("denied"))));
    assert!(!ran_at_all(&result(ToolStatus::Skipped, Some("skipped"))));
}

/// 回合 `turn` 里的第 `seq` 条压缩，没写原因的（当作到线了）。
fn compaction(seq: u64, turn: u64) -> Event {
    compaction_of(seq, turn, None)
}

/// 同上，原因是 `trigger`。
fn compaction_of(seq: u64, turn: u64, trigger: Option<CompactTrigger>) -> Event {
    let seq = Seq::new(seq).expect("序号合写法");
    Event {
        seq,
        at: Timestamp::from_unix_millis(0).expect("在范围里"),
        turn: Seq::new(turn).map(TurnId::new),
        by: By::Kernel,
        cause: None,
        body: Body::ContextCompacted(ContextCompacted {
            upto: Seq::FIRST,
            summary: "S".to_string(),
            trigger,
            instructions: None,
            notes: String::new(),
            restored: Vec::new(),
            refills: None,
        }),
    }
}

#[test]
fn only_compactions_in_the_undone_turns_count() {
    let log = [compaction(4, 3), compaction(6, 5), compaction(7, 5)];
    let turns = |seqs: &[u64]| -> Vec<TurnId> {
        seqs.iter()
            .filter_map(|n| Seq::new(*n).map(TurnId::new))
            .collect()
    };
    assert_eq!(
        compactions(&log, &turns(&[5]), false),
        2,
        "一轮里压过两次的是 2"
    );
    assert_eq!(compactions(&log, &turns(&[3, 5]), false), 3);
    assert_eq!(compactions(&log, &turns(&[8]), false), 0);
}

/// 清空另数（施工 6-8 补）：数压缩的不算它，数清空的只算它；几种压缩都不是清空。
#[test]
fn clears_are_counted_apart_from_compactions() {
    let log = [
        compaction_of(4, 3, Some(CompactTrigger::Manual)),
        compaction_of(6, 5, Some(CompactTrigger::Clear)),
        compaction_of(8, 7, Some(CompactTrigger::Overflow)),
        compaction_of(10, 9, Some(CompactTrigger::Clear)),
        compaction(12, 11),
    ];
    let turns: Vec<TurnId> = [3, 5, 7, 9, 11]
        .iter()
        .filter_map(|n| Seq::new(*n).map(TurnId::new))
        .collect();
    assert_eq!(compactions(&log, &turns, false), 3);
    assert_eq!(compactions(&log, &turns, true), 2);
    assert_eq!(
        compactions(&log, &turns[1..2], false),
        0,
        "只撤了清空那一轮"
    );
}

/// 一个用完就删的临时目录：装这几个测试要存的 blob（施工 4-7 再补）。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-undo-report-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建得了");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 第 `seq` 条：一条工具结果，效果是 `effects`。
fn tool_result(seq: u64, effects: Vec<Effect>) -> Event {
    let seq = Seq::new(seq).expect("不是 0");
    Event {
        seq,
        at: Timestamp::from_unix_millis(0).expect("在范围里"),
        turn: None,
        by: By::Kernel,
        cause: None,
        body: Body::ToolResult(ToolResult {
            call_id: CallId::new(seq, 1).expect("编号合写法"),
            status: ToolStatus::Ok,
            blocks: Vec::new(),
            duration_ms: None,
            human: None,
            effects,
        }),
    }
}

/// `path` 上改了一个文件的效果：改前是 `before`（`None` 是新建的），改后是 `after`。
fn changed(path: &str, before: Option<ContentHash>, after: ContentHash) -> Effect {
    Effect::FileChanged(FileChanged {
        path: path.to_string(),
        before,
        after,
    })
}

/// 改回的一步：照第 `result` 条的第 `effect` 个效果，在 `path` 上做 `action`，结局是 `outcome`。
fn restored_step(
    result: u64,
    effect: u32,
    path: &str,
    action: RestoreAction,
    outcome: RestoreOutcome,
) -> Restored {
    Restored {
        result: Seq::new(result).expect("不是 0"),
        effect,
        path: path.to_string(),
        action,
        outcome,
        found: None,
        trash: None,
        hash: None,
        error: None,
    }
}

/// `[&str]` 写成 `Vec<String>`，拿来和差异比。
fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(|line| (*line).to_string()).collect()
}

/// 撤销时改回了内容（`outcome` 是 `restored`、`action` 是 `write`）的，附上她改完的对改回以后的差异，连同新增、
/// 删掉的行数（施工 4-7 再补）。
#[test]
fn a_restored_write_gets_a_diff_of_before_and_after() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.0.clone());
    let before = blobs.put(b"old\n").expect("存得下");
    let after = blobs.put(b"new\n").expect("存得下");
    let log = [tool_result(10, vec![changed("/w/a", Some(before), after)])];
    let file = restored_step(
        10,
        0,
        "/w/a",
        RestoreAction::Write,
        RestoreOutcome::Restored,
    );
    let diff = entry(&file, &log, &blobs, true).diff.expect("有差异");
    assert_eq!(diff.diff, lines(&["@@ -1 +1 @@", "-new", "+old"]));
    assert_eq!((diff.added, diff.removed), (1, 1));
    assert_eq!(diff.more, 0);
}

/// 恢复时反过来：对照的是撤销以后的（改前）对改回以后的（改后）。
#[test]
fn a_restored_write_in_the_redo_direction_compares_the_other_way() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.0.clone());
    let before = blobs.put(b"old\n").expect("存得下");
    let after = blobs.put(b"new\n").expect("存得下");
    let log = [tool_result(10, vec![changed("/w/a", Some(before), after)])];
    let file = restored_step(
        10,
        0,
        "/w/a",
        RestoreAction::Write,
        RestoreOutcome::Restored,
    );
    let diff = entry(&file, &log, &blobs, false).diff.expect("有差异");
    assert_eq!(diff.diff, lines(&["@@ -1 +1 @@", "-old", "+new"]));
}

/// 新建的文件改回以前没有内容：照空的算，不是没有差异（施工 4-7 再补）。
#[test]
fn a_restored_write_with_no_earlier_content_diffs_from_empty() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.0.clone());
    let after = blobs.put(b"a\nb\n").expect("存得下");
    let log = [tool_result(12, vec![changed("/w/n", None, after)])];
    let file = restored_step(
        12,
        0,
        "/w/n",
        RestoreAction::Write,
        RestoreOutcome::Restored,
    );
    let diff = entry(&file, &log, &blobs, false).diff.expect("有差异");
    assert_eq!(diff.added, 2, "{diff:?}");
    assert_eq!(diff.removed, 0, "{diff:?}");
}

/// `trash`、`untrash` 只是挪位置，内容没变：不带差异（施工 4-7 再补）。
#[test]
fn trash_and_untrash_never_get_a_diff() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.0.clone());
    let after = blobs.put(b"x\n").expect("存得下");
    let log = [tool_result(14, vec![changed("/w/n", None, after)])];
    let trashed = restored_step(
        14,
        0,
        "/w/n",
        RestoreAction::Trash,
        RestoreOutcome::Restored,
    );
    assert!(entry(&trashed, &log, &blobs, true).diff.is_none());
    let untrashed = restored_step(
        14,
        0,
        "/w/n",
        RestoreAction::Untrash,
        RestoreOutcome::Restored,
    );
    assert!(entry(&untrashed, &log, &blobs, true).diff.is_none());
}

/// 「现在已经是要改成的样子」（本来就一样，没写）的：两边内容一样，没有差异，不带（施工 4-7 再补）。
#[test]
fn a_restored_write_already_matching_gets_no_diff() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.0.clone());
    let same = blobs.put(b"same\n").expect("存得下");
    let log = [tool_result(
        10,
        vec![changed("/w/a", Some(same.clone()), same)],
    )];
    let file = restored_step(
        10,
        0,
        "/w/a",
        RestoreAction::Write,
        RestoreOutcome::Restored,
    );
    assert!(entry(&file, &log, &blobs, true).diff.is_none());
}

/// 太大的不带差异：`restored` 的也照同一条字节上限算（施工 4-7 再补）。
#[test]
fn a_restored_write_too_big_to_diff_is_silent() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.0.clone());
    let before = blobs.put(&vec![b'x'; (1 << 20) + 1]).expect("存得下");
    let after = blobs.put(b"y\n").expect("存得下");
    let log = [tool_result(10, vec![changed("/w/a", Some(before), after)])];
    let file = restored_step(
        10,
        0,
        "/w/a",
        RestoreAction::Write,
        RestoreOutcome::Restored,
    );
    assert!(entry(&file, &log, &blobs, true).diff.is_none());
}

/// 改了很多行的：`more` 只算截断以后的，`added`、`removed` 照整份差异数（施工 4-7 再补）。
#[test]
fn a_long_restored_diff_counts_added_and_removed_from_the_whole_thing() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.0.clone());
    let long: String = (0..30).map(|n| format!("line {n}\n")).collect();
    let before = blobs.put(b"x\n").expect("存得下");
    let after = blobs.put(long.as_bytes()).expect("存得下");
    let log = [tool_result(10, vec![changed("/w/a", Some(before), after)])];
    let file = restored_step(
        10,
        0,
        "/w/a",
        RestoreAction::Write,
        RestoreOutcome::Restored,
    );
    let diff = entry(&file, &log, &blobs, false).diff.expect("有差异");
    assert_eq!(diff.diff.len(), 20, "{diff:?}");
    assert_eq!(
        diff.more, 12,
        "差异一共 32 行：一行 @@、一行删掉的、30 行加上的"
    );
    assert_eq!(diff.added, 30, "{diff:?}");
    assert_eq!(diff.removed, 1, "{diff:?}");
}

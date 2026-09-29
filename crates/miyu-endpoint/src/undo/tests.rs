//! 撤销回应里「执行过几条命令」怎么认跑过的（施工 4-9 再补一）：成了、出错的算；可能跑了一半的（跑到一半被打断、
//! 重启时没跑完）算；被拒的、没跑过的、跳过的，和别的已取消不算。「撤掉了几次压缩」只数带着撤掉的回合的（施工 6-9）。

use miyu_kernel::event::{Body, ContextCompacted, Event, Said, ToolResult, ToolStatus};
use miyu_kernel::id::{CallId, Seq, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;

use super::{compactions, ran_at_all};

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

/// 回合 `turn` 里的第 `seq` 条压缩。
fn compaction(seq: u64, turn: u64) -> Event {
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
            trigger: None,
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
    assert_eq!(compactions(&log, &turns(&[5])), 2, "一轮里压过两次的是 2");
    assert_eq!(compactions(&log, &turns(&[3, 5])), 3);
    assert_eq!(compactions(&log, &turns(&[8])), 0);
}

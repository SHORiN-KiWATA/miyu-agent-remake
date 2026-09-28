//! 撤销回应里「执行过几条命令」怎么认跑过的（施工 4-9 再补一）：成了、出错的算；可能跑了一半的（跑到一半被打断、
//! 重启时没跑完）算；被拒的、没跑过的、跳过的，和别的已取消不算。

use miyu_kernel::event::{Said, ToolResult, ToolStatus};
use miyu_kernel::id::{CallId, Seq};

use super::ran_at_all;

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

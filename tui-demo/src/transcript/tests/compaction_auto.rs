//! 自动压缩和手动压缩分开画（蓝图 `tui.md`「正文」第 9 条，2026-10-10 项目主人）：停下来等的那一行绿点、不画进度条，
//! 手动的照旧有；提前在后台压好换上的正文不写。

use super::super::Transcript;
use super::apply;
use crate::core::{Compaction, Push};

fn shown(t: &Transcript) -> Vec<String> {
    t.entries
        .iter()
        .filter(|e| !e.hidden)
        .map(|e| e.text.clone())
        .collect()
}

#[test]
fn an_automatic_wait_is_a_green_dot_line_without_a_bar_and_my_own_compact_keeps_it() {
    // 「正文」第 9 条（2026-10-10 项目主人：自动压缩停下等时绿点「正在压缩上下文…」，不要进度条；手动的照旧有）。
    let auto = |written| {
        Push::Compaction(Compaction::Progress {
            written,
            expected: Some(20000),
            manual: None,
        })
    };
    let mut t = Transcript::default();
    apply(&mut t, vec![Push::TurnStarted(1, None), auto(0), auto(800)]);
    let entry = t.entries.iter().find(|e| e.progress.is_some()).unwrap();
    assert_eq!(
        entry.progress.as_ref().unwrap().expected,
        None,
        "不画进度条"
    );
    assert_eq!(entry.mark.as_deref(), Some("● "), "行首绿点");
    // 这个头自己发了 `/compact`（旧核心不带 `trigger`）：照手动画进度条。
    let mut t = Transcript {
        manual_compaction: true,
        ..Transcript::default()
    };
    apply(&mut t, vec![Push::TurnStarted(1, None), auto(0)]);
    let entry = t.entries.iter().find(|e| e.progress.is_some()).unwrap();
    assert_eq!(entry.progress.as_ref().unwrap().expected, Some(20000));
    assert_eq!(entry.mark, None);
}

#[test]
fn a_summary_prepared_in_the_background_leaves_no_line() {
    // 「正文」第 9 条（2026-10-10 项目主人：「异步压缩的时候是不需要有任何提示的……文中不要有」，提示另弹）。
    let mut t = Transcript::default();
    let swapped = Push::Compaction(Compaction::Done {
        before: 101_000,
        after: 42_000,
        prepared: true,
    });
    apply(&mut t, vec![Push::TurnStarted(1, None), swapped]);
    assert!(shown(&t).is_empty(), "{:?}", shown(&t));
    // 停下来等过（来过进度）的照旧换成结果行。
    let mut t = Transcript::default();
    let waited = Push::Compaction(Compaction::Progress {
        written: 0,
        expected: Some(20000),
        manual: None,
    });
    let swapped = Push::Compaction(Compaction::Done {
        before: 101_000,
        after: 42_000,
        prepared: true,
    });
    apply(&mut t, vec![Push::TurnStarted(1, None), waited, swapped]);
    assert_eq!(shown(&t).len(), 1);
}

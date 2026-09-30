//! 正文里还留着几个附件（蓝图 `tui.md`「输入框」第 12 条）：每一种各数各的，撤销藏起来的不算、恢复了又算，`/new` 从头数。

use std::collections::HashMap;

use super::apply;
use crate::core::{EndReason, Push};
use crate::transcript::{Chip, Transcript};

fn said(t: &mut Transcript, turn: u64, text: &str, kinds: &[(&str, &str)]) {
    let chips = kinds
        .iter()
        .map(|(label, kind)| Chip {
            label: (*label).into(),
            full: (*label).into(),
            kind: Some((*kind).into()),
        })
        .collect();
    t.user(text.into(), chips);
    apply(
        t,
        vec![
            Push::TurnStarted(turn, Some(turn)),
            Push::TurnEnded(EndReason::Completed),
        ],
    );
}

fn counts(pairs: &[(&str, usize)]) -> HashMap<String, usize> {
    pairs.iter().map(|(k, n)| ((*k).to_string(), *n)).collect()
}

#[test]
fn attachments_are_counted_per_kind_and_undone_ones_give_their_numbers_back() {
    let mut t = Transcript::default();
    said(&mut t, 1, "[图片 1]", &[("[图片 1]", "image")]);
    said(
        &mut t,
        2,
        "[图片 2][PDF 1]",
        &[("[图片 2]", "image"), ("[PDF 1]", "pdf")],
    );
    // 粘贴块不算附件。
    t.user(
        "[已粘贴 12 行]".into(),
        vec![Chip {
            label: "[已粘贴 12 行]".into(),
            full: "长文".into(),
            kind: None,
        }],
    );
    assert_eq!(t.attachment_counts(), counts(&[("image", 2), ("pdf", 1)]));
    apply(&mut t, vec![Push::Reverted(vec![2])]);
    assert_eq!(
        t.attachment_counts(),
        counts(&[("image", 1)]),
        "撤掉的号收回来"
    );
    apply(&mut t, vec![Push::Unreverted(vec![2])]);
    assert_eq!(
        t.attachment_counts(),
        counts(&[("image", 2), ("pdf", 1)]),
        "恢复了又算"
    );
    t.fresh();
    assert!(t.attachment_counts().is_empty(), "/new 从头数");
}

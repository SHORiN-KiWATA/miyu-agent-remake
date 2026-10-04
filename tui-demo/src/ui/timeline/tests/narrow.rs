//! 窄屏下的收起那一行（蓝图 `tui.md`「窗口小的时候」第 4 条）：只有一条命令的短标题截掉加 `…`，次数和用时留着；
//! 别的整行截掉加 `…`。

use std::time::Instant;

use serde_json::json;
use unicode_width::UnicodeWidthStr;

use super::{command, segment, thought};
use crate::core::ToolStatus;
use crate::transcript::StepKind;
use crate::ui::test_support::Fixture;
use crate::ui::timeline::rows;

#[test]
fn a_narrow_folded_line_clips_the_title_and_keeps_the_time() {
    let f = Fixture::new();
    let t0 = Instant::now();
    let mut ls = command(t0, 1, ToolStatus::Ok);
    if let StepKind::Tool { parsed, .. } = &mut ls.kind {
        *parsed =
            json!({"command": "ls", "description": "List current directory contents of the tree"});
    }
    let seg = segment(vec![thought(t0, 0), ls], None);
    let mut ctx = f.ctx();
    ctx.width = 30;
    let line = rows(0, &seg, &ctx)[0].line.to_string();
    assert!(line.width() <= 30, "{line:?} 超出了 30 列");
    assert!(line.contains('…'), "短标题截掉加 …：{line:?}");
    assert!(
        line.trim_end().ends_with(" · 1 thought · 2s"),
        "次数和用时留着：{line:?}"
    );
}

#[test]
fn a_narrow_count_line_is_clipped_at_its_end() {
    let f = Fixture::new();
    let t0 = Instant::now();
    let seg = segment(
        vec![
            thought(t0, 0),
            command(t0, 1, ToolStatus::Ok),
            command(t0, 2, ToolStatus::Ok),
        ],
        None,
    );
    let mut ctx = f.ctx();
    ctx.width = 20;
    let line = rows(0, &seg, &ctx)[0].line.to_string();
    assert!(line.width() <= 20, "{line:?} 超出了 20 列");
    assert!(line.trim_end().ends_with('…'), "{line:?}");
}

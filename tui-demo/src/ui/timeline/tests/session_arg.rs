//! 参数里带着会话编号的一步（蓝图 `tui.md`「时间线」第 8 条）：核心 C-4 起 `history` 能翻别的会话，对象后面跟「会话 短编号」。

use std::time::Instant;

use serde_json::json;

use super::{rows, segment, step, text, thought};
use crate::core::ToolStatus;
use crate::transcript::{StepKind, ToolState};
use crate::ui::test_support::Fixture;

#[test]
fn a_step_on_another_session_names_it_by_its_short_id() {
    use miyu_store::resources::ResourceRoot;
    let mut f = Fixture::new();
    let root = ResourceRoot::at(concat!(env!("CARGO_MANIFEST_DIR"), "/../resources"));
    f.human = miyu_store::human::Human::load(&root, "zh").unwrap();
    let t0 = Instant::now();
    let history = |args: serde_json::Value| {
        step(
            StepKind::Tool {
                name: "history".into(),
                args: String::new(),
                parsed: args,
                state: ToolState::Done(ToolStatus::Ok),
                output: String::new(),
                said: None,
            },
            t0,
            0,
            1,
        )
    };
    let title = |args| {
        // 只有一步的点开直接铺内容、不写标题（「时间线」）：前面垫一步思考。
        let steps = vec![thought(t0, 0), history(args)];
        text(&rows(0, &segment(steps, Some(true)), &f.ctx())).join("\n")
    };
    let other = json!({"query": "暗号", "session": "0192f3a0-1111-7abc-8def-001122334455"});
    let shown = title(other);
    assert!(shown.contains("翻记录 · 暗号 · 会话 22334455"), "{shown}");
    let here = title(json!({"query": "暗号"}));
    assert!(
        here.contains("翻记录 · 暗号") && !here.contains("会话"),
        "{here}"
    );
}

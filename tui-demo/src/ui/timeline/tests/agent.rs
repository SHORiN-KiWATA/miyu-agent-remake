//! 派子代理那一步（蓝图 `tui.md`「时间线」第 8、14 条）。

use std::time::Instant;

use serde_json::json;

use super::{rows, segment, step, text, thought};
use crate::core::ToolStatus;
use crate::transcript::{StepKind, ToolState};
use crate::ui::test_support::Fixture;

#[test]
fn an_agent_step_says_the_job_then_the_description_and_opens_on_the_prompt() {
    // 2026-09-30 项目主人：原来是「派子代理 · 描述 · 派出去了：j10」，点开是结果那一句；改成「派子代理 · j10 · 描述」，
    // 点开看完整的交代。
    use miyu_store::resources::ResourceRoot;
    let mut f = Fixture::new();
    let root = ResourceRoot::at(concat!(env!("CARGO_MANIFEST_DIR"), "/../resources"));
    f.human = miyu_store::human::Human::load(&root, "zh").unwrap();
    let t0 = Instant::now();
    let said = miyu_kernel::event::Said {
        key: "software/basesystem/agent/started".into(),
        fields: [
            ("job".to_string(), "j10".to_string()),
            ("title".to_string(), "查文档".to_string()),
        ]
        .into_iter()
        .collect(),
    };
    let mut agent = step(
        StepKind::Tool {
            name: "agent".into(),
            args: String::new(),
            parsed: json!({"description": "查文档", "prompt": "先读 a.md\n再用一句话总结"}),
            state: ToolState::Done(ToolStatus::Ok),
            output: "Started subagent j10: \"查文档\".".into(),
            said: Some(said),
        },
        t0,
        0,
        1,
    );
    agent.open = Some(true);
    let lines = text(&rows(
        0,
        &segment(vec![thought(t0, 0), agent], Some(true)),
        &f.ctx(),
    ));
    assert!(
        lines.iter().any(|l| l.ends_with("派子代理 · j10 · 查文档")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("先读 a.md")),
        "点开是交代的活：{lines:?}"
    );
    assert!(lines.iter().any(|l| l.contains("再用一句话总结")));
    assert!(
        !lines.iter().any(|l| l.contains("Started subagent")),
        "结果那一句不再写"
    );
}

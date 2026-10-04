//! 派子代理那一步（蓝图 `tui.md`「时间线」第 8、14 条）。

use std::time::Instant;

use serde_json::json;

use super::{command, rows, segment, step, text, thought};
use crate::core::ToolStatus;
use crate::transcript::{StepKind, ToolState};
use crate::ui::test_support::Fixture;

#[test]
fn an_agent_step_says_the_job_then_the_description_and_opens_on_the_prompt() {
    // 2026-09-30 项目主人：原来是「派子代理 · 描述 · 派出去了：j10」，点开是结果那一句；改成「派子代理 · j10 · 描述」，
    // 点开看完整的交代。
    let mut f = Fixture::new();
    f.human = crate::human::Human::from_resources("zh");
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
            name: "subagent".into(),
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

/// 一步工具：名字 `name`，做成了。
fn tool(name: &str, t0: Instant, start: u64) -> crate::transcript::Step {
    step(
        StepKind::Tool {
            name: name.into(),
            args: String::new(),
            parsed: json!({"description": "查文档", "prompt": "看看"}),
            state: ToolState::Done(ToolStatus::Ok),
            output: String::new(),
            said: None,
        },
        t0,
        start,
        1,
    )
}

#[test]
fn a_folded_segment_that_spawned_agents_does_not_say_it_used_tools() {
    // 2026-10-01 项目主人：派子代理的一段原来收成 `Used 1 tool`；照网页演示的写法，派子代理写 Spawned，留言写 Messaged。
    let f = Fixture::new();
    let t0 = Instant::now();
    let fold = |steps| text(&rows(0, &segment(steps, None), &f.ctx()));
    assert_eq!(
        fold(vec![thought(t0, 0), tool("subagent", t0, 1)]),
        ["  Spawned 1 agent · 1 thought · 2s"]
    );
    assert_eq!(
        fold(vec![tool("subagent", t0, 0), tool("subagent", t0, 1)]),
        ["  Spawned 2 agents · 2s"]
    );
    assert_eq!(
        fold(vec![tool("agent", t0, 0), tool("message_agent", t0, 1)]),
        ["  Used 2 tools · 2s"],
        "旧名不认（2026-10-01 项目主人：不留兼容），算别的工具"
    );
    assert_eq!(
        fold(vec![tool("send_message", t0, 0)]),
        ["  Messaged 1 agent · 1s"]
    );
    assert_eq!(
        fold(vec![
            command(t0, 0, ToolStatus::Ok),
            command(t0, 1, ToolStatus::Ok),
            tool("subagent", t0, 2),
            tool("send_message", t0, 3),
            tool("read", t0, 4)
        ]),
        ["  Ran 2 commands · 1 agent · 1 message · 1 tool · 5s"],
        "打头的是命令，后面依次子代理、留言、别的工具"
    );
    assert_eq!(
        fold(vec![tool("subagent", t0, 0), tool("read", t0, 1)]),
        ["  Spawned 1 agent · 1 tool · 2s"]
    );
}

/// 留言一步：`send_message`（C-5 改的名字），发给 `to`。
fn message(to: &str, t0: Instant, start: u64) -> crate::transcript::Step {
    step(
        StepKind::Tool {
            name: "send_message".into(),
            args: String::new(),
            parsed: json!({"to": to, "message": "测试过了"}),
            state: ToolState::Done(ToolStatus::Ok),
            output: String::new(),
            said: None,
        },
        t0,
        start,
        1,
    )
}

#[test]
fn messages_to_other_sessions_are_counted_apart_from_messages_to_agents() {
    // 2026-10-01 项目主人定分开数：C-5 起 `send_message` 能发给别的会话，原来会数成子代理。
    let f = Fixture::new();
    let t0 = Instant::now();
    let fold = |steps| text(&rows(0, &segment(steps, None), &f.ctx()));
    let other = "0192f3a0-1111-7abc-8def-001122334455";
    assert_eq!(
        fold(vec![message("j10", t0, 0)]),
        ["  Messaged 1 agent · 1s"],
        "子代理照旧，新名字 send_message 也算留言"
    );
    assert_eq!(
        fold(vec![message(other, t0, 0)]),
        ["  Messaged 1 session · 1s"]
    );
    assert_eq!(
        fold(vec![message("parent", t0, 0), message("22334455", t0, 1)]),
        ["  Messaged 1 agent · Messaged 1 session · 2s"],
        "父会话算子代理那一格，8 位后缀也认得是会话"
    );
    assert_eq!(
        fold(vec![command(t0, 0, ToolStatus::Ok), message(other, t0, 1)]),
        ["  列目录 · Messaged 1 session · 2s"],
        "不打头也写 Messaged（项目主人认的样子）；只有一条命令、有短标题时用短标题打头（2026-10-02 项目主人定）"
    );
}

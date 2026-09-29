//! 量尺（`cargo test ruler -- --ignored --nocapture`）：思考越长、会话越长，排一帧要多久。只打数字，不断言耗时。

use std::time::{Duration, Instant};

use serde_json::json;

use crate::core::ToolStatus;
use crate::transcript::{Segment, Step, StepKind, ToolState};
use crate::ui::test_support::Fixture;
use crate::ui::timeline::rows;

fn step(kind: StepKind, t0: Instant, start: u64, took: u64) -> Step {
    let mut s = Step::new(kind);
    s.started = t0 + Duration::from_secs(start);
    s.took = Some(Duration::from_secs(took));
    s
}

fn command(t0: Instant, start: u64, status: ToolStatus) -> Step {
    step(
        StepKind::Tool {
            name: "shell".into(),
            args: String::new(),
            parsed: json!({"command": "ls", "description": "列目录"}),
            state: ToolState::Done(status),
            output: "a.txt".into(),
            said: None,
        },
        t0,
        start,
        1,
    )
}

fn segment(steps: Vec<Step>, open: Option<bool>) -> Segment {
    let mut s = Segment::new();
    s.steps = steps;
    s.finished = true;
    s.open = open;
    s
}

/// 思考越长，排一帧时间线要多久。
#[test]
#[ignore]
fn ruler_thought() {
    let f = Fixture::new();
    for chars in [2_000usize, 20_000, 100_000, 300_000] {
        let line = "我们需要想清楚这个问题的每一步，然后再给出结论，顺便检查一下边界情况。\n";
        let text: String = line.repeat(chars / line.chars().count() + 1);
        let mut seg = segment(vec![Step::new(StepKind::Thought { text })], None);
        seg.finished = false;
        let t = Instant::now();
        let n = 20;
        for _ in 0..n {
            std::hint::black_box(rows(0, &seg, &f.ctx()));
        }
        println!(
            "思考 {chars} 字：一帧 {:.2}ms",
            t.elapsed().as_secs_f64() * 1000.0 / n as f64
        );
        // 最坏的：一整段不换行。
        let flat: String = "想".repeat(chars);
        let mut seg = segment(vec![Step::new(StepKind::Thought { text: flat })], None);
        seg.finished = false;
        let t = Instant::now();
        for _ in 0..n {
            std::hint::black_box(rows(0, &seg, &f.ctx()));
        }
        println!(
            "  一整段不换行：一帧 {:.2}ms",
            t.elapsed().as_secs_f64() * 1000.0 / n as f64
        );
    }
}

/// 长会话里正文一帧排成行要多久：整份重排和按条缓存对比。
#[test]
#[ignore]
fn ruler_body() {
    use crate::transcript::{Kind, Transcript};
    let f = Fixture::new();
    let t0 = Instant::now();
    let think =
        "我们需要想清楚这个问题的每一步，然后再给出结论，顺便检查一下边界情况。\n".repeat(120);
    let reply = "## 结论\n\n- 第一点：**要紧的**先说\n- 第二点：`code` 放这里\n\n一段普通的回答文字，稍微长一点，好折几行。\n".repeat(20);
    for turns in [10usize, 40, 100] {
        let mut t = Transcript::default();
        for _ in 0..turns {
            t.note(Kind::User, "帮我看看这个目录".into());
            t.note(Kind::Steps, String::new());
            let mut seg = segment(
                vec![
                    step(
                        StepKind::Thought {
                            text: think.clone(),
                        },
                        t0,
                        0,
                        3,
                    ),
                    command(t0, 3, ToolStatus::Ok),
                    command(t0, 4, ToolStatus::Ok),
                ],
                None,
            );
            seg.finished = true;
            t.entries.last_mut().unwrap().segment = Some(seg);
            t.note(Kind::Reply, reply.clone());
        }
        let ctx = f.ctx();
        let fresh = Instant::now();
        let n = crate::ui::test_support::fresh_rows(&t.entries, &ctx).len();
        let whole = fresh.elapsed();
        let cache = std::cell::RefCell::new(crate::ui::row_cache::RowCache::default());
        let cold = Instant::now();
        crate::ui::row_cache::build(&t.entries, &ctx, &cache);
        let cold = cold.elapsed();
        let warm = Instant::now();
        for _ in 0..10 {
            std::hint::black_box(crate::ui::row_cache::build(&t.entries, &ctx, &cache));
        }
        println!(
            "{turns} 轮（{n} 行）：整份重排 {:.2}ms；缓存头一帧 {:.2}ms，之后每帧 {:.3}ms",
            whole.as_secs_f64() * 1000.0,
            cold.as_secs_f64() * 1000.0,
            warm.elapsed().as_secs_f64() * 100.0
        );
    }
}

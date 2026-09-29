//! 正文按条缓存排好的行（蓝图「正文」第 8 条）：和整份重排一模一样；没变的条目不重排，变了的、在进行的才排；
//! 条目按编号认，不按位置；图做好了全部重排。

use std::cell::RefCell;
use std::time::{Duration, Instant};

use serde_json::json;

use super::{RowCache, build};
use crate::core::ToolStatus;
use crate::transcript::{JobMark, Kind, Segment, Step, StepKind, ToolState, Transcript};
use crate::ui::rows::Target;
use crate::ui::test_support::{Fixture, fresh_rows};

fn finished_segment(t0: Instant) -> Segment {
    let mut thought = Step::new(StepKind::Thought {
        text: "先看看目录".into(),
    });
    thought.started = t0;
    thought.took = Some(Duration::from_secs(1));
    let mut shell = Step::new(StepKind::Tool {
        name: "shell".into(),
        args: String::new(),
        parsed: json!({"command": "ls", "description": "列目录"}),
        state: ToolState::Done(ToolStatus::Ok),
        output: "a.txt".into(),
        said: None,
    });
    shell.started = t0;
    shell.took = Some(Duration::from_secs(1));
    let mut seg = Segment::new();
    seg.steps = vec![thought, shell];
    seg.finished = true;
    seg
}

/// 四条：你说的话、一段时间线、回答、一条后台通知。
fn sample() -> Transcript {
    let mut t = Transcript::default();
    t.note(Kind::User, "你好".into());
    t.note(Kind::Steps, String::new());
    t.entries.last_mut().unwrap().segment = Some(finished_segment(Instant::now()));
    t.note(Kind::Reply, "## 标题\n\n- 一\n- 二\n\n一段话。".into());
    t.job(JobMark::Done, "后台命令完成 · ls".into(), "a.txt".into());
    t
}

fn lines(rows: impl Iterator<Item = String>) -> Vec<String> {
    rows.collect()
}

#[test]
fn cached_rows_match_a_fresh_build() {
    let f = Fixture::new();
    let t = sample();
    let ctx = f.ctx();
    let cache = RefCell::new(RowCache::default());
    let fresh = fresh_rows(&t.entries, &ctx);
    for _ in 0..2 {
        let cached = build(&t.entries, &ctx, &cache);
        assert_eq!(cached.len(), fresh.len());
        assert_eq!(
            lines(cached.iter().map(|r| r.line.to_string())),
            lines(fresh.iter().map(|r| r.line.to_string()))
        );
        let targets: Vec<Option<Target>> = cached.iter().map(|r| r.target).collect();
        assert_eq!(targets, fresh.iter().map(|r| r.target).collect::<Vec<_>>());
        assert_eq!(
            cached.get(fresh.len() - 1).unwrap().plain,
            fresh.last().unwrap().plain
        );
    }
}

#[test]
fn only_changed_entries_are_rebuilt() {
    let f = Fixture::new();
    let mut t = sample();
    let mut ctx = f.ctx();
    let cache = RefCell::new(RowCache::default());
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 4, "头一帧全排");
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 0, "什么都没变：一条都不排");
    t.entries[3].open = true;
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 1, "点开了一条：只排它");
    ctx.hover = Some(Target::Segment(1));
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 1, "悬停在一条上：只排它");
    ctx.hover = Some(Target::Entry(3));
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 2, "悬停挪了：离开的和进来的");
    t.entries[2].text.push_str("\n\n又一段。");
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 1, "回答长了：只排它");
    ctx.width = 40;
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 4, "宽度变了：全排");
}

#[test]
fn a_live_segment_is_rebuilt_every_frame() {
    let f = Fixture::new();
    let mut t = sample();
    t.entries[1]
        .segment
        .as_mut()
        .unwrap()
        .steps
        .push(Step::new(StepKind::Thought {
            text: "还在想".into(),
        }));
    t.entries[1].segment.as_mut().unwrap().finished = false;
    let ctx = f.ctx();
    let cache = RefCell::new(RowCache::default());
    build(&t.entries, &ctx, &cache);
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 1, "转圈、走表的那一段每帧重排");
}

#[test]
fn entries_are_known_by_id_not_position() {
    let f = Fixture::new();
    let mut t = sample();
    let ctx = f.ctx();
    let cache = RefCell::new(RowCache::default());
    build(&t.entries, &ctx, &cache);
    // 排队退回的消息会从中间抽走：后面的条目挪了位置，点中的东西要跟着对。
    t.entries.remove(0);
    let cached = build(&t.entries, &ctx, &cache);
    let fresh = fresh_rows(&t.entries, &ctx);
    let targets: Vec<Option<Target>> = cached.iter().map(|r| r.target).collect();
    assert_eq!(targets, fresh.iter().map(|r| r.target).collect::<Vec<_>>());
}

#[test]
fn a_finished_figure_rebuilds_everything() {
    let f = Fixture::new();
    let t = sample();
    let ctx = f.ctx();
    let cache = RefCell::new(RowCache::default());
    build(&t.entries, &ctx, &cache);
    // 图做好了、被扔掉了：占几行变了，全排一遍。
    ctx.figures.borrow_mut().forget();
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 4);
}

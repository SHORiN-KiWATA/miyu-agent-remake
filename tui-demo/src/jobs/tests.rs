//! 后台任务表、待办和假数据源（蓝图 `tui.md`「后台命令、子代理和侧边栏」）。

use std::time::{Duration, Instant};

use super::{Board, Feed, JobKind, JobState, TodoState};
use crate::config::Config;

fn at(t0: Instant, ms: u64) -> Instant {
    t0 + Duration::from_millis(ms)
}

#[test]
fn a_fake_shell_prints_line_by_line_and_ends_with_its_exit_code() {
    let script = Config::builtin().unwrap().fake;
    let (mut feed, mut board) = (Feed::default(), Board::default());
    let t0 = Instant::now();
    feed.start_shell(&script, &mut board, t0);
    assert_eq!(board.shells_running(), 1);
    let s = &script.shells[0];
    feed.advance(&script, &mut board, at(t0, s.every_ms * 2));
    assert_eq!(board.jobs[0].output.len(), 1, "一次只推一步");
    feed.advance(&script, &mut board, at(t0, s.every_ms * 2));
    assert_eq!(board.jobs[0].output.len(), 2, "落后了下一帧接着推");
    let mut finished = Vec::new();
    for n in 0..=s.lines.len() as u64 {
        finished.extend(feed.advance(&script, &mut board, at(t0, s.every_ms * (n + 2))));
    }
    assert_eq!(board.jobs[0].output.len(), s.lines.len());
    assert_eq!(board.jobs[0].state, JobState::Done);
    assert_eq!(finished, vec![board.jobs[0].id], "结束只报一次");
    assert!(feed.next_at().is_none());
    // 第二条照脚本失败。
    feed.start_shell(&script, &mut board, t0);
    let s = &script.shells[1];
    for n in 0..=s.lines.len() as u64 {
        feed.advance(&script, &mut board, at(t0, s.every_ms * (n + 1)));
    }
    assert_eq!(board.jobs[1].state, JobState::Failed(s.exit));
}

#[test]
fn a_stopped_shell_gets_no_more_output() {
    let script = Config::builtin().unwrap().fake;
    let (mut feed, mut board) = (Feed::default(), Board::default());
    let t0 = Instant::now();
    feed.start_shell(&script, &mut board, t0);
    let id = board.jobs[0].id;
    assert!(board.stop(id, at(t0, 10)));
    assert!(!board.stop(id, at(t0, 20)), "停过的不再停");
    let finished = feed.advance(&script, &mut board, at(t0, 60_000));
    assert!(finished.is_empty());
    assert!(board.jobs[0].output.is_empty());
    assert_eq!(board.jobs[0].state, JobState::Stopped);
    assert_eq!(board.shells_running(), 0);
    assert_eq!(board.shells().len(), 1, "结束了的还列在面板里");
}

#[test]
fn a_fake_agent_moves_on_and_reports() {
    let script = Config::builtin().unwrap().fake;
    let (mut feed, mut board) = (Feed::default(), Board::default());
    let t0 = Instant::now();
    feed.start_agent(&script, &mut board, t0);
    let a = &script.agents[0];
    assert_eq!(board.agents().len(), 1);
    assert_eq!(board.jobs[0].kind, JobKind::Agent);
    assert_eq!(board.jobs[0].doing, a.steps[0]);
    feed.advance(&script, &mut board, at(t0, a.every_ms));
    assert_eq!(board.jobs[0].doing, a.steps[1]);
    assert_eq!(board.jobs[0].tokens, a.tokens_per_step * 2);
    for n in 2..=a.steps.len() as u64 {
        feed.advance(&script, &mut board, at(t0, a.every_ms * n));
    }
    assert_eq!(board.jobs[0].state, JobState::Done);
    assert_eq!(board.jobs[0].report, a.report, "交回报告");
    assert!(board.agents().is_empty(), "回报了就不在状态行里");
}

#[test]
fn a_fake_todo_list_advances_one_item_at_a_time() {
    let script = Config::builtin().unwrap().fake;
    let (mut feed, mut board) = (Feed::default(), Board::default());
    let t0 = Instant::now();
    feed.start_todo(&script, &mut board, t0);
    let n = script.todos.items.len();
    assert_eq!(board.todo_progress(), Some((0, n)));
    assert_eq!(board.todos[0].state, TodoState::Active);
    feed.advance(&script, &mut board, at(t0, script.todos.every_ms));
    assert_eq!(board.todos[0].state, TodoState::Done);
    assert_eq!(board.todos[1].state, TodoState::Active);
    assert_eq!(board.todo_progress(), Some((1, n)));
    for k in 2..=n as u64 {
        feed.advance(&script, &mut board, at(t0, script.todos.every_ms * k));
    }
    assert_eq!(board.todo_progress(), Some((n, n)));
}

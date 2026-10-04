//! 后台任务表照核心的事件记、待办的假数据源（蓝图 `tui.md`「后台命令、子代理和侧边栏」）。

use std::time::{Duration, Instant};

use super::{Board, Feed, JobKind, JobState, TodoState};
use crate::config::Config;
use crate::core::{JobEnd, JobReason, JobStart};

fn at(t0: Instant, ms: u64) -> Instant {
    t0 + Duration::from_millis(ms)
}

fn started(job: &str, agent: bool) -> JobStart {
    JobStart {
        call_id: "call_1".into(),
        job: job.into(),
        agent,
        title: "跑测试".into(),
        session: agent.then(|| "s-child".into()),
    }
}

fn ended(job: &str, reason: JobReason, exit_code: Option<i32>, signal: Option<u32>) -> JobEnd {
    JobEnd {
        job: job.into(),
        reason,
        exit_code,
        signal,
        duration_ms: Some(20_000),
        text: "查完了".into(),
    }
}

#[test]
fn a_command_starts_with_its_own_text_and_ends_by_its_exit() {
    let t0 = Instant::now();
    let mut board = Board::default();
    let id = board.start(&started("j1", false), Some("cargo test".into()), t0);
    assert_eq!(board.jobs[0].title, "cargo test", "命令写命令本身");
    assert_eq!(board.shells_running(), 1);
    assert_eq!(
        board.end(&ended("j1", JobReason::Finished, Some(0), None), at(t0, 50)),
        Some(id)
    );
    assert_eq!(board.jobs[0].state, JobState::Done);
    assert_eq!(
        board.jobs[0].elapsed(at(t0, 99_000)),
        Duration::from_secs(20),
        "用时照核心给的"
    );
    let cases = [
        (
            ended("j2", JobReason::Finished, Some(1), None),
            JobState::Failed(1),
        ),
        (
            ended("j2", JobReason::Finished, None, Some(9)),
            JobState::Killed(9),
        ),
        (
            ended("j2", JobReason::Stopped, None, None),
            JobState::Stopped,
        ),
        (ended("j2", JobReason::Undone, None, None), JobState::Undone),
        (
            ended("j2", JobReason::Restarted, None, None),
            JobState::Restarted,
        ),
    ];
    for (end, state) in cases {
        let mut board = Board::default();
        board.start(&started("j2", false), None, t0);
        assert_eq!(board.jobs[0].title, "跑测试", "拿不到命令的写标题");
        board.end(&end, t0);
        assert_eq!(board.jobs[0].state, state);
    }
    assert_eq!(
        board.end(&ended("j9", JobReason::Stopped, None, None), t0),
        None,
        "对不上的不管"
    );
}

#[test]
fn an_agent_reports_back_and_runs_again_after_a_message() {
    let t0 = Instant::now();
    let mut board = Board::default();
    board.start(&started("j2.1", true), None, t0);
    let agent = &board.jobs[0];
    assert_eq!(agent.kind, JobKind::Agent);
    assert_eq!(
        (agent.title.as_str(), agent.session.as_deref()),
        ("跑测试", Some("s-child"))
    );
    assert!(board.agent_mut("s-child").is_some());
    board.end(&ended("j2.1", JobReason::Finished, None, None), t0);
    assert_eq!(board.jobs[0].state, JobState::Done);
    assert_eq!(board.jobs[0].report, "查完了", "交回来的回答");
    assert!(board.agents().is_empty() && !board.busy());
    board.messaged("j2.1");
    assert_eq!(board.agents().len(), 1, "留了言又在跑");
    assert!(board.busy());
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

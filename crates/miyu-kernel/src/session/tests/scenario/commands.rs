//! 后台命令的内核这一半（施工 7-3，`docs/blueprint/kernel/session.md`「载入和崩溃」「有计划的重启」，`agents.md` 第八条）：
//! 用过的最大任务编号，撤掉的回合里的也算；载入时没结束的后台命令补 `aborted`，`by` 是内核，子代理不补，结束过的不补，
//! 只记下、不开轮，排在崩了的那一轮收尾前面；要重启了以后到的结束只记下、不开轮，再起来接着干的那一轮看得到。

use super::reports::{CHILD, dispatched, event, last_request, starter};
use super::*;
use crate::event::{JobReason, JobReported};

/// 日志里的 `job.reported`：几号、哪个任务、为什么。
fn reported(s: &Stage) -> Vec<(u64, String, JobReason)> {
    s.log()
        .iter()
        .filter_map(|event| match &event.body {
            Body::JobReported(reported) => Some((
                event.seq.get(),
                reported.job.to_string(),
                reported.reason.clone(),
            )),
            _ => None,
        })
        .collect()
}

fn body(s: &Stage, n: u64) -> &JobReported {
    match &event(s, n).body {
        Body::JobReported(reported) => reported,
        other => panic!("第 {n} 条应该是 job.reported：{other:?}"),
    }
}

#[test]
fn the_last_job_number_counts_undone_turns_too() {
    let s = stage();
    assert_eq!(s.last_job_number(), 0, "没派过的是 0");
    let mut s = dispatched(stage());
    assert_eq!(s.last_job_number(), 2);
    s.revert(TurnId::new(seq(3)));
    assert_eq!(s.last_job_number(), 2, "撤掉的回合里派的也算");
    s.crash();
    assert_eq!(s.last_job_number(), 2, "载入以后照日志算回来");
}

#[test]
fn loading_after_a_crash_ends_running_commands_as_aborted() {
    let mut s = dispatched(stage());
    s.crash();
    assert_eq!(
        reported(&s),
        [(13, "j2".to_string(), JobReason::Aborted)],
        "只补后台命令，子代理 {CHILD} 不补"
    );
    let aborted = event(&s, 13);
    assert_eq!(aborted.by, By::Kernel);
    assert_eq!(aborted.cause, None);
    assert_eq!(aborted.turn, None);
    let body = body(&s, 13);
    assert_eq!(
        (body.duration_ms, &body.output, body.chars, body.exit_code),
        (None, &None, None, None),
        "进程什么时候没的不知道"
    );
    assert_eq!(s.turns().len(), 1, "只记下，不开轮");
    s.crash();
    assert_eq!(reported(&s).len(), 1, "结束过了，再载入不再补");
}

#[test]
fn a_command_that_ended_is_not_aborted() {
    let mut s = dispatched(stage());
    s.model([Line::says("测试过了。")]);
    s.job_ends(2, JobReason::Exited, starter(), Some(id(1)));
    s.crash();
    assert_eq!(reported(&s), [(13, "j2".to_string(), JobReason::Exited)]);
}

#[test]
fn a_crash_mid_turn_ends_the_commands_before_the_turn() {
    let mut s = stage();
    s.model([
        Line::calls("先放出去。", &[("shell", "{}")]),
        Line::says("等着。").held(),
    ]);
    s.tools([Play::starts_command(1, "跑测试")]);
    s.say("跑测试");
    s.crash();
    let told = story(&s);
    let aborted = told
        .iter()
        .position(|line| line.contains("job.reported"))
        .expect("补了结束");
    let ended = told
        .iter()
        .position(|line| line.contains("turn.ended:aborted"))
        .expect("那一轮收尾");
    assert!(aborted < ended, "先补结束，再收尾：{told:#?}");
}

#[test]
fn after_restarting_a_command_ending_only_records_and_the_resumed_turn_hears_it() {
    let mut s = dispatched(stage());
    s.model([Line::says("好").held()]);
    s.say("还在吗");
    s.restarting();
    let turns = s.turns().len();
    s.job_ends(2, JobReason::Exited, starter(), Some(id(1)));
    assert_eq!(s.turns().len(), turns, "要关了，不开轮");
    let (seq, _, reason) = reported(&s)[0].clone();
    assert_eq!(reason, JobReason::Exited);
    s.model([Line::says("看到测试过了。")]);
    s.crash();
    assert!(
        s.turns().len() > turns,
        "被重启打断的那一轮接着干：{:#?}",
        story(&s)
    );
    assert!(
        last_request(&s).contains(&format!("{seq} job.reported\n")),
        "接着干的那一轮看得到它"
    );
    assert_eq!(reported(&s).len(), 1, "结束过了，不补 aborted");
}

//! 压缩里的任务（施工 7-8，`docs/blueprint/compaction.md` 第三条第 2 条、第八条，`agents.md` 第十条）：检查点里代码写的
//! 几段多一段，照编号列还在跑的任务（编号、种类、标题），结束了的、派它的那一轮撤掉了的不列，没有的不写；还没听到的
//! 回报算这一轮要回应的，不压进摘要。那一段照出厂的两份模板写，和样本 `docs/designs/samples/reports/checkpoint-jobs.txt`
//! 一字不差。

use super::reports::CHILD;
use super::*;
use crate::event::{ChildReason, ContextCompacted, JobReason};
use crate::id::JobId;
use crate::session::{Compaction, Notes, RunningNotes};
use crate::template::Template;

/// 出厂的那两份模板：头一行、一个任务一行。
const HEAD: &str = include_str!("../../../../../../resources/core/compaction/notes-jobs.txt");
const ITEM: &str = include_str!("../../../../../../resources/core/compaction/notes-job.txt");
/// 样本：还在跑的一个子代理、一个后台命令。
const SAMPLE: &str =
    include_str!("../../../../../../docs/designs/samples/reports/checkpoint-jobs.txt");

/// 会压缩的替身：输出预留、余量各 10，尾巴 0，不重读；检查点里代码写的几段用测试的模板，还在跑的那一段用出厂的。
fn compacting() -> Stage {
    let make = || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            tail: 0,
            price: crate::estimate::Flat {
                image: 50,
                file: 50,
            },
            rebuild: None,
            pause: None,
            shorten: None,
            isolate: false,
        });
        let template = |source: &str| Template::parse(source).unwrap();
        policy.notes = Some(Notes {
            files: template("<files>\n"),
            files_more: template("<more {count}/>\n"),
            retrieve: template("<retrieve {upto}/>\n"),
            too_large: template("<too-large {files}/>\n"),
            uncovered: None,
            running: Some(RunningNotes {
                head: template(HEAD),
                item: template(ITEM),
            }),
        });
        policy
    };
    Stage::new(make, environment("/w"), at(0))
}

/// 第一轮（3 号）派出去一个子代理 `j1`、一个后台命令 `j2`，说完了（报 5000，到 12 号）。
fn dispatched() -> Stage {
    let mut s = compacting();
    s.model([
        Line::calls("派出去。", &[("shell", "{}"), ("shell", "{}")]),
        Line::says("派出去了。").reports(5_000),
    ]);
    s.tools([
        Play::starts_agent(1, "查 CI", CHILD),
        Play::starts_command(2, "跑测试"),
    ]);
    s.say("查一下 CI，顺便跑测试");
    s
}

/// 交窗口 120，下一轮一开头就压：摘要是 S1。
fn next_turn(s: &mut Stage) {
    s.limits(Some(120), None);
    s.model([Line::says("S1"), Line::says("嗯。")]);
    s.say("接着来");
}

/// 写下的那一条压缩。
fn compacted(s: &Stage) -> &ContextCompacted {
    s.log()
        .iter()
        .find_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted),
            _ => None,
        })
        .expect("压了")
}

#[test]
fn the_checkpoint_lists_the_jobs_still_running_as_the_sample() {
    let mut s = dispatched();
    next_turn(&mut s);
    let compacted = compacted(&s);
    assert_eq!(
        compacted.notes,
        format!("<retrieve {}/>\n{SAMPLE}", compacted.upto),
        "取回指路后面接还在跑的那一段"
    );
}

#[test]
fn ended_and_undone_jobs_are_not_listed_and_none_writes_nothing() {
    let mut s = dispatched();
    s.model([Line::says("看到了。").reports(5_000)]);
    s.child_reports(1, CHILD, ChildReason::Done, "做完了。");
    next_turn(&mut s);
    assert!(
        compacted(&s).notes.ends_with("- j2 command \"跑测试\"\n"),
        "报过 done 的子代理不列：{:?}",
        compacted(&s).notes
    );

    let mut s = dispatched();
    s.revert(TurnId::new(seq(3)));
    s.model([Line::says("好。").reports(5_000)]);
    s.say("换个话题");
    next_turn(&mut s);
    let notes = &compacted(&s).notes;
    assert!(
        !notes.contains("Jobs still running"),
        "派它们的那一轮撤掉了：她看不到，不列，也就没有这一段：{notes:?}"
    );
}

#[test]
fn a_report_not_yet_heard_stays_after_the_checkpoint() {
    let mut s = dispatched();
    // 要重启了停掉的只记下，没人听到过。
    s.job_ends(2, JobReason::Restarted, By::Kernel, None);
    let report = s.log().last().unwrap().seq;
    next_turn(&mut s);
    let compacted = compacted(&s);
    assert!(
        compacted.upto < report,
        "还没听到的回报算这一轮要回应的，压到它前面为止：upto {}，回报 {report}",
        compacted.upto
    );
    let request = &s.requests().last().unwrap().1;
    assert!(
        listed_request(request).contains(&format!("{report} job.reported")),
        "压完的请求里回报原样在：{}",
        listed_request(request)
    );
}

/// 撤到压缩以前派它们的那一轮（施工 6-9：先读回日志）：读回来记下撤销的同时照样停它们。
#[test]
fn an_undo_that_reads_back_the_log_stops_them_too() {
    let mut s = dispatched();
    next_turn(&mut s);
    let undo = s.revert(TurnId::new(seq(3)));
    assert!(!s.read_backs().is_empty(), "撤到了压缩以前，先读回");
    let stop = crate::session::Action::StopJobs {
        jobs: vec![JobId::new(1).unwrap(), JobId::new(2).unwrap()],
        by: alice(),
        cause: undo,
    };
    assert_eq!(s.stopping(), [stop]);
}

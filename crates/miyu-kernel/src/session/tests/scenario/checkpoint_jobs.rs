//! 压缩里的任务（施工 7-8，`docs/blueprint/compaction.md` 第三条第 2 条、第八条，`agents.md` 第十条）：还没听到的回报算
//! 这一轮要回应的，不压进摘要（还没听到的空了的通知也是，施工 C-6）；撤到压缩以前也停派出去的。检查点里代码写的几段不列还在跑的任务：7-8 加过那一段，
//! 2026-10-01 实测摘要记得住，照「非必要不加」去掉了（施工 7-8 补）。

use super::reports::CHILD;
use super::*;
use crate::event::{ContextCompacted, JobReason};
use crate::id::JobId;
use crate::session::{Compaction, Notes};
use crate::template::Template;

/// 会压缩的替身：输出预留、余量各 10，尾巴 0，不重读；检查点里代码写的几段用测试的模板。
fn compacting() -> Stage {
    let make = || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            line_percent: 100,
            margin_percent: 100,
            tail: 0,
            lead: 0,
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
            todos: None,
            uncovered: None,
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

/// 一个子代理、一个后台命令都还在跑：代码写的几段只有取回指路，不列它们（施工 7-8 补）。
#[test]
fn the_checkpoint_does_not_list_the_jobs_still_running() {
    let mut s = dispatched();
    next_turn(&mut s);
    let compacted = compacted(&s);
    assert_eq!(
        compacted.notes,
        format!("<retrieve {}/>\n", compacted.upto),
        "只有取回指路"
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

/// 空了的通知一样（施工 C-6，`compaction.md` 第三条第 2 条）：作废了的只记下，没人听到过，压到它前面为止。
#[test]
fn a_notice_not_yet_heard_stays_after_the_checkpoint() {
    let peer = "0192f3a0-2222-7abc-8def-5566899aa000";
    let mut s = compacting();
    s.model([
        Line::calls("订了。", &[("read", "{}")]),
        Line::says("等它。").reports(5_000),
    ]);
    s.tools([Play::watches(peer)]);
    s.say("它做完告诉我");
    let [(_, since)] = s.watching().try_into().unwrap();
    let due = since.unix_millis() + 12 * 3_600_000;
    s.watch_ends_at(
        peer,
        crate::event::IdleReason::Expired,
        crate::time::Timestamp::from_unix_millis(due).unwrap(),
    );
    let notice = s.log().last().unwrap().seq;
    assert!(matches!(s.log().last().unwrap().body, Body::PeerIdle(_)));
    next_turn(&mut s);
    let compacted = compacted(&s);
    assert!(
        compacted.upto < notice,
        "还没听到的通知算这一轮要回应的，压到它前面为止：upto {}，通知 {notice}",
        compacted.upto
    );
    let request = &s.requests().last().unwrap().1;
    assert!(
        listed_request(request).contains(&format!("{notice} peer.idle")),
        "压完的请求里通知原样在：{}",
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
        undone: true,
        jobs: vec![JobId::new(1).unwrap(), JobId::new(2).unwrap()],
        by: alice(),
        cause: undo,
    };
    assert_eq!(s.stopping(), [stop]);
}

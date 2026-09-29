//! 事件的样本文件（`docs/designs/03-事件模型.md` 第三节「样本文件」）：内核认识的每种事件
//! 都有一份；一份里的每一行读进来再写出去一字不差、认得出种类、种类和文件名对得上；
//! 几份样本讲的是同一个会话，序号不重复，时间跟着序号不往回走：一条输入产生的几条事件，时刻相同
//! （`02-内核.md` 第六节「回合怎么开、请求怎么发」第 3 条）。只有带 `parent` 的那一条 `session.created` 例外：它是
//! 样本会话派的子代理自己的日志里的第 1 条（施工 7-1），和派它的 `job.started`、它的回报对得上。
//!
//! 样本是图纸的一部分，住在设计文档旁边，所以这个测试要读文件。`src/` 里的测试不许 I/O
//! （纯逻辑门禁只扫 `src/`），集成测试可以。

use std::fs;
use std::path::PathBuf;

use miyu_kernel::event::{Body, ChildReported, Effect, Event, JobKind, JobStarted};
use miyu_kernel::origin::By;

/// 样本所在的目录：这个 crate 的目录往上两级是仓库根。
fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/designs/samples/events")
}

/// 目录里每一份样本的每一行：种类名（文件名去掉 `.jsonl`）和日志里的那一行（去掉行尾的换行）。
/// 一份样本写这一种事件在样本会话里的每一条，一行一条，以一个换行结尾，和日志文件一样。
fn samples() -> Vec<(String, String)> {
    let dir = samples_dir();
    let entries =
        fs::read_dir(&dir).unwrap_or_else(|e| panic!("读不了样本目录 {}：{e}", dir.display()));
    let mut samples = Vec::new();
    for entry in entries {
        let path = entry.expect("列样本目录时出错").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let kind = name
            .strip_suffix(".jsonl")
            .unwrap_or_else(|| panic!("样本目录里只放 .jsonl：{}", path.display()));
        let text =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("读不了 {}：{e}", path.display()));
        let lines = text
            .strip_suffix('\n')
            .unwrap_or_else(|| panic!("{kind} 的样本要以一个换行结尾"));
        for line in lines.split('\n') {
            assert!(!line.is_empty(), "{kind} 的样本里不能有空行");
            samples.push((kind.to_string(), line.to_string()));
        }
    }
    samples.sort();
    samples
}

#[test]
fn every_sample_round_trips_as_its_own_kind() {
    for (kind, line) in samples() {
        let event =
            Event::from_line(&line).unwrap_or_else(|e| panic!("{kind} 的样本读不出来：{e}"));
        assert!(
            !matches!(event.body, Body::Unknown { .. }),
            "{kind} 的样本应该认得出种类"
        );
        assert_eq!(event.body.kind(), kind, "样本的文件名和里面写的种类对不上");
        assert_eq!(event.to_line(), line, "{kind} 的样本写出去和原文不一样");
    }
}

#[test]
fn every_known_kind_has_a_sample() {
    let kinds: Vec<String> = samples().into_iter().map(|(kind, _)| kind).collect();
    for kind in Body::KINDS {
        assert!(
            kinds.iter().any(|k| k == kind),
            "{kind} 没有样本，要加一份 docs/designs/samples/events/{kind}.jsonl"
        );
    }
}

/// 每一份样本的每一条事件。
fn events() -> Vec<Event> {
    samples()
        .into_iter()
        .map(|(kind, line)| {
            Event::from_line(&line).unwrap_or_else(|e| panic!("{kind} 的样本读不出来：{e}"))
        })
        .collect()
}

/// 子会话的第一条：带着父会话的 `session.created`。它在子会话自己的日志里，不是样本会话的。
fn in_the_child_log(event: &Event) -> bool {
    matches!(&event.body, Body::SessionCreated(created) if created.parent.is_some())
}

#[test]
fn samples_tell_one_session_in_order() {
    let mut events: Vec<Event> = events()
        .into_iter()
        .filter(|event| !in_the_child_log(event))
        .collect();
    events.sort_by_key(|event| event.seq);
    for pair in events.windows(2) {
        let (earlier, later) = (&pair[0], &pair[1]);
        assert!(earlier.seq < later.seq, "两份样本的序号都是 {}", later.seq);
        assert!(
            earlier.at <= later.at,
            "序号 {} 的时间不能早于序号 {}",
            later.seq,
            earlier.seq
        );
    }
}

/// 子代理的几条对得上（施工 7-1）：派它的 `job.started` 记着它的会话；它的回报写的就是那个会话，`by` 是它；它自己日志的
/// 第一条带着父会话、是第 1 层，比派它的那条结果早。后台命令的回报对得上一个 `command`。
#[test]
fn the_child_in_the_samples_is_the_one_the_parent_started() {
    let events = events();
    let started: Vec<(&Event, &JobStarted)> = events
        .iter()
        .flat_map(|event| match &event.body {
            Body::ToolResult(result) => result
                .effects
                .iter()
                .filter_map(|effect| match effect {
                    Effect::JobStarted(started) => Some((event, started)),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        })
        .collect();
    let started_as = |job, what: JobKind| {
        started
            .iter()
            .find(|(_, started)| started.job == job && started.what == what)
            .copied()
            .unwrap_or_else(|| panic!("样本里没有派 {job} 的 {}", what.as_str()))
    };
    let reports: Vec<(&Event, &ChildReported)> = events
        .iter()
        .filter_map(|event| match &event.body {
            Body::ChildReported(reported) => Some((event, reported)),
            _ => None,
        })
        .collect();
    assert!(!reports.is_empty(), "样本里要有子代理的回报");
    for (event, reported) in reports {
        let (_, started) = started_as(reported.job, JobKind::Agent);
        assert_eq!(started.session.as_ref(), Some(&reported.session));
        assert!(
            matches!(&event.by, By::Session(by) if by.id == reported.session),
            "回报的 by 要是那个子会话：{:?}",
            event.by
        );
    }
    let child: Vec<&Event> = events.iter().filter(|e| in_the_child_log(e)).collect();
    let [child] = child.as_slice() else {
        panic!("样本里要正好有一条子会话的 session.created：{child:?}");
    };
    let Body::SessionCreated(created) = &child.body else {
        unreachable!("in_the_child_log 只认 session.created");
    };
    assert_eq!(created.depth, Some(1), "样本会话是主会话，它派的是第 1 层");
    assert_eq!(child.seq.get(), 1, "子会话日志的第一条");
    assert!(
        matches!(&child.by, By::Session(by) if Some(&by.id) == created.parent.as_ref()),
        "子会话由父会话造"
    );
    let (spawned, _) = started
        .iter()
        .find(|(_, started)| started.what == JobKind::Agent)
        .copied()
        .expect("样本里派过子代理");
    assert!(child.at <= spawned.at, "子会话造好了，派它的那次调用才返回");
    for event in &events {
        if let Body::JobReported(reported) = &event.body {
            started_as(reported.job, JobKind::Command);
        }
    }
}

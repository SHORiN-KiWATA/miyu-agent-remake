//! 名册（施工 7-4）：照事件样本记下派出去的两个任务和它们的回报；列出来的是还在跑的和最近结束的几个，用时照回报、照到的
//! 时刻、照这一刻。

use super::*;

/// 事件样本里 `kinds` 这几种的每一条：派出去的后台命令 `j1`（07:52:03.200）和子代理 `j2`（07:52:03.420），它们的回报。
fn samples(kinds: &[&str]) -> Vec<Event> {
    [
        include_str!("../../../../../docs/designs/samples/events/tool.result.jsonl"),
        include_str!("../../../../../docs/designs/samples/events/job.reported.jsonl"),
        include_str!("../../../../../docs/designs/samples/events/child.reported.jsonl"),
    ]
    .concat()
    .lines()
    .filter(|line| {
        kinds
            .iter()
            .any(|kind| line.contains(&format!("\"kind\":\"{kind}\"")))
    })
    .map(|line| Event::from_line(line).expect("样本读得进来"))
    .collect()
}

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("合写法")
}

fn millis(text: &str) -> u64 {
    u64::try_from(at(text).unix_millis()).expect("1970 年以后")
}

#[test]
fn started_jobs_run_until_their_report_arrives() {
    let roster = Roster::from_events(&samples(&["tool.result"]));
    let brief: Vec<(String, String, Option<String>, u64)> = roster
        .list(at("2026-09-25T07:53:03.200Z"))
        .into_iter()
        .map(|listed| {
            (
                listed.job.to_string(),
                listed.what.as_str().to_string(),
                listed.ended,
                listed.took_ms,
            )
        })
        .collect();
    assert_eq!(
        brief,
        [
            ("j1".to_string(), "command".to_string(), None, 60_000),
            ("j2".to_string(), "agent".to_string(), None, 59_780),
        ],
        "还在跑的算到这一刻"
    );
    assert_eq!(roster.running().len(), 2);
    let ended = Roster::from_events(&samples(&["tool.result", "job.reported", "child.reported"]));
    let listed = ended.list(at("2026-09-26T00:00:00.000Z"));
    assert_eq!(listed[0].ended.as_deref(), Some("exited"));
    assert_eq!(listed[0].took_ms, 81_234, "后台命令照回报里的用时");
    assert_eq!(listed[1].ended.as_deref(), Some("done"));
    assert_eq!(
        listed[1].took_ms,
        millis("2026-09-25T07:55:40.000Z") - millis("2026-09-25T07:52:03.420Z"),
        "子代理从派出去算到回报到的那一刻"
    );
    assert!(ended.running().is_empty());
    // 回报里的用时是进程自己的：和两条事件的时刻差不一样时照它。
    let mut quick = Roster::from_events(&samples(&["tool.result"]));
    quick.note(
        &Event::from_line(
            r#"{"seq":120,"at":"2026-09-25T08:00:00.000Z","kind":"job.reported","by":{"kind":"kernel"},"body":{"job":"j1","reason":"restarted","duration_ms":5}}"#,
        )
        .expect("写法对"),
    );
    assert_eq!(quick.list(at("2026-09-26T00:00:00.000Z"))[0].took_ms, 5);
    let j1 = ended.get(JobId::new(1).expect("从 1 数起")).expect("记着");
    assert!(
        j1.end.as_ref().and_then(|end| end.output.clone()).is_some(),
        "记着输出的 blob"
    );
}

#[test]
fn only_the_latest_ended_jobs_are_listed() {
    let mut roster = Roster::default();
    for n in 1..=8u64 {
        let started = format!(
            r#"{{"seq":{n},"at":"2026-09-25T07:00:0{n}.000Z","kind":"tool.result","turn":1,"by":{{"kind":"tool","call_id":"call_1_{n}"}},"body":{{"call_id":"call_1_{n}","status":"ok","blocks":[],"effects":[{{"kind":"job.started","job":"j{n}","what":"command","title":"t{n}"}}]}}}}"#
        );
        roster.note(&Event::from_line(&started).expect("写法对"));
    }
    // j8 还在跑；j1 到 j7 结束，结束的先后倒过来：j7 最先、j1 最后。
    for n in 1..=7u64 {
        let ended = format!(
            r#"{{"seq":{},"at":"2026-09-25T08:00:{:02}.000Z","kind":"job.reported","by":{{"kind":"kernel"}},"body":{{"job":"j{n}","reason":"aborted"}}}}"#,
            10 + n,
            10 - n
        );
        roster.note(&Event::from_line(&ended).expect("写法对"));
    }
    let jobs: Vec<String> = roster
        .list(at("2026-09-25T09:00:00.000Z"))
        .iter()
        .map(|listed| listed.job.to_string())
        .collect();
    assert_eq!(
        jobs,
        ["j1", "j2", "j3", "j4", "j5", "j8"],
        "还在跑的全列，结束了的只列最近结束的五个，照编号排"
    );
}

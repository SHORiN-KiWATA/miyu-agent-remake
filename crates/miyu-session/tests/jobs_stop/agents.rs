//! 子代理的那几条（施工 7-4）：她停掉它，回报记成它交来的、不叫醒她；人停它叫醒她；父会话停下时连它派的一起停、都不叫醒。

use miyu_kernel::event::ChildReason;
use miyu_kernel::origin::Session;

use super::*;

#[tokio::test]
async fn she_stops_a_subagent_it_reports_stopped_and_she_is_not_woken() {
    let home = Home::new();
    let held = Held::new(&[]);
    let agent =
        serde_json::json!({"description": "查导出", "prompt": "Read src/lib.rs."}).to_string();
    let (output, stop) = (jobs("output", Some("j1")), jobs("stop", Some("j1")));
    let script = Script::new([
        Play::calls(&[("agent", agent.as_str())]),
        Play::Says("派出去了。"),
        Play::calls(&[("jobs", output.as_str())]),
        Play::calls(&[("jobs", stop.as_str())]),
        Play::Says("停了。"),
    ]);
    let table = Arc::new(Table::default());
    let handle = session(&home, &script, &held, &table).await;
    talk(&handle, "cmd-1", "派一个").await;
    talk(&handle, "cmd-2", "停掉它").await;
    let log = home.log(handle.id());
    assert_eq!(
        result_text(&log, 1),
        format!(
            "{:?}",
            text_blocks("Half of the tests read.\n(j1 is still running. It is using read now.)\n")
        ),
        "最近的回答和这一步在跑的工具"
    );
    assert_eq!(
        result_text(&log, 2),
        format!("{:?}", text_blocks("Stopped j1.\n"))
    );
    let parent = handle.id().clone();
    let child = SessionId::parse(CHILD).expect("合写法");
    assert_eq!(
        table.stopped(),
        [(
            child.clone(),
            CommandId::parse(&format!("{parent}/j1/stop")).expect("合写法"),
            By::Session(Session { id: parent.clone() })
        )],
        "经会话表停下子会话，记成父会话发的"
    );
    let [(event, reported)] = child_reported(&log)[..] else {
        panic!("记了一条回报：{:?}", kinds(&log))
    };
    assert_eq!(
        reported,
        &ChildReported {
            job: j(1),
            session: child.clone(),
            reason: ChildReason::Stopped,
            text: "Half of the tests read.".to_string(),
            truncated: false,
            person: false,
            by_model: true,
        }
    );
    assert_eq!(event.by, By::Session(Session { id: child }));
    assert_eq!(
        event.cause,
        Some(CommandId::parse(&format!("{parent}/j1/stopped")).expect("合写法"))
    );
    assert_eq!(turns(&log), 2, "她停的不叫醒她");
}

#[tokio::test]
async fn a_person_stopping_a_subagent_wakes_her() {
    let home = Home::new();
    let held = Held::new(&[]);
    let agent = serde_json::json!({"description": "查导出", "prompt": "Read."}).to_string();
    let script = Script::new([
        Play::calls(&[("agent", agent.as_str())]),
        Play::Says("派出去了。"),
        Play::Says("它被停了。"),
    ]);
    let table = Arc::new(Table {
        quiet: true,
        ..Table::default()
    });
    let handle = session(&home, &script, &held, &table).await;
    talk(&handle, "cmd-1", "派一个").await;
    let stopped = within("人停", handle.stop_job(j(1), alice(), id("stop-1")))
        .await
        .expect("会话在跑");
    assert_eq!(stopped, Ok(()));
    let log = until_logged(&home, handle.id(), |log| turns(log) == 2).await;
    let [(_, reported)] = child_reported(&log)[..] else {
        panic!("记了一条回报：{:?}", kinds(&log))
    };
    assert!(!reported.by_model, "人停的");
    assert_eq!(
        reported.text, "",
        "这一轮还没说过话：正文空着，不拿上一轮的"
    );
    assert_eq!(
        handle
            .stop_job(j(1), alice(), id("stop-2"))
            .await
            .expect("在跑"),
        Err(JobError::Ended),
        "停过的不再停"
    );
    assert_eq!(table.stopped().len(), 1, "停过的不再去停子会话");
}

#[tokio::test]
async fn stopping_a_session_stops_what_it_started_without_waking_it() {
    let home = Home::new();
    let held = Held::new(&["x\n"]);
    let agent = serde_json::json!({"description": "查导出", "prompt": "Read."}).to_string();
    let script = Script::new([
        Play::calls(&[("start", "{}"), ("agent", agent.as_str())]),
        Play::Says("派出去了。"),
    ]);
    let table = Arc::new(Table::default());
    let handle = session(&home, &script, &held, &table).await;
    talk(&handle, "cmd-1", "都派出去").await;
    let parent = By::Session(Session {
        id: SessionId::parse("01a0d78c-ca52-7d19-8b64-000000000001").expect("合写法"),
    });
    within("全停", handle.stop_jobs(parent.clone(), id("p/j4/stop")))
        .await
        .expect("会话在跑");
    let log = until_logged(&home, handle.id(), |log| {
        !job_reported(log).is_empty() && !child_reported(log).is_empty()
    })
    .await;
    let [(event, reported)] = job_reported(&log)[..] else {
        panic!("{:?}", kinds(&log))
    };
    assert!(reported.by_model, "连着父会话一起停的，不叫醒");
    assert_eq!(
        (event.by.clone(), event.cause.clone()),
        (parent, Some(id("p/j4/stop")))
    );
    let [(_, child)] = child_reported(&log)[..] else {
        panic!("{:?}", kinds(&log))
    };
    assert!(child.by_model && child.reason == ChildReason::Stopped);
    assert_eq!(table.stopped().len(), 1, "它派的子代理也停了");
    assert_eq!(held.killed(), 1);
    assert_eq!(turns(&log), 1, "都不叫醒它");
}

#[tokio::test]
async fn a_long_answer_is_cut_like_an_upward_report() {
    let home = Home::new();
    let held = Held::new(&[]);
    let agent = serde_json::json!({"description": "查导出", "prompt": "Read."}).to_string();
    let stop = jobs("stop", Some("j1"));
    let script = Script::new([
        Play::calls(&[("agent", agent.as_str())]),
        Play::Says("派出去了。"),
        Play::calls(&[("jobs", stop.as_str())]),
        Play::Says("停了。"),
    ]);
    let table = Arc::new(Table {
        long: Some(40_000),
        ..Table::default()
    });
    let handle = session(&home, &script, &held, &table).await;
    talk(&handle, "cmd-1", "派一个").await;
    talk(&handle, "cmd-2", "停掉它").await;
    let log = home.log(handle.id());
    let [(_, reported)] = child_reported(&log)[..] else {
        panic!("记了一条回报：{:?}", kinds(&log))
    };
    // 和子会话自己向上回报同一份截法（`Reports::cut`）：头尾各一半，中间一行省了多少个字。
    let half = "字".repeat(15_000);
    assert!(reported.truncated);
    assert_eq!(
        reported.text,
        format!("{half}\n[... 10000 characters omitted ...]\n{half}")
    );
}

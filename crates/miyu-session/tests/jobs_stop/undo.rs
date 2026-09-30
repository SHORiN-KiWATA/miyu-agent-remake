//! 撤销停掉那一轮派出去的，执行器这一头（施工 7-8，`docs/blueprint/agents.md` 第七条第 1、3 条）：后台命令整组杀掉、记
//! `undone`（`by`、`cause` 是撤销的人和命令），子代理经会话表停下、回报记成它交来的 `undone`；都不叫醒她；已经结束了的不再
//! 杀；恢复撤销不重起。

use miyu_kernel::event::ChildReason;
use miyu_kernel::origin::Session;

use super::*;

/// 派出去一条后台命令 `j1`、一个子代理 `j2`，这一轮说完了。
async fn dispatched(home: &Home, held: &Arc<Held>, table: &Arc<Table>) -> Handle {
    let agent = serde_json::json!({"description": "查 CI", "prompt": "Find why CI is red."});
    let agent = agent.to_string();
    let script = Script::new([
        Play::calls(&[("start", "{}"), ("agent", agent.as_str())]),
        Play::Says("放出去了。"),
    ]);
    let handle = session(home, &script, held, table).await;
    talk(&handle, "cmd-1", "后台跑，再派一个").await;
    handle
}

#[tokio::test]
async fn undoing_the_turn_stops_its_command_and_subagent_without_waking_her() {
    let home = Home::new();
    let held = Held::new(&["building\n"]);
    let table = Arc::new(Table::default());
    let handle = dispatched(&home, &held, &table).await;
    let undo = Command::Revert { turn: None };
    assert!(matches!(
        ask(&handle, "cmd-2", undo).await,
        Ok(Outcome::Accepted { .. })
    ));
    assert_eq!(held.killed(), 1, "回应之前后台命令已经整组杀了");
    let log = until_logged(&home, handle.id(), |log| {
        job_reported(log).len() == 1 && child_reported(log).len() == 1
    })
    .await;
    let [(event, reported)] = job_reported(&log)[..] else {
        unreachable!()
    };
    assert_eq!(
        (&reported.job, &reported.reason, reported.by_model),
        (&j(1), &JobReason::Undone, false)
    );
    assert_eq!(
        (&event.by, event.cause.as_ref(), event.turn),
        (&alice(), Some(&id("cmd-2")), None),
        "by、cause 是撤销的人和命令，不带回合编号"
    );
    assert!(reported.chars == Some(9), "到这时的输出存下了");
    let parent = handle.id().clone();
    let child = SessionId::parse(CHILD).expect("合写法");
    assert_eq!(
        table.stopped(),
        [(
            child.clone(),
            CommandId::parse(&format!("{parent}/j2/stop")).expect("合写法"),
            By::Session(Session { id: parent.clone() })
        )],
        "经会话表停下子会话，连它派的一起停"
    );
    let [(event, reported)] = child_reported(&log)[..] else {
        unreachable!()
    };
    assert_eq!(
        (&reported.job, &reported.reason, reported.by_model),
        (&j(2), &ChildReason::Undone, false)
    );
    assert_eq!(
        (&event.by, event.cause.as_ref()),
        (
            &By::Session(Session { id: child }),
            Some(&CommandId::parse(&format!("{parent}/j2/undone")).expect("合写法"))
        ),
        "子代理的回报记成它交来的"
    );
    assert_eq!(turns(&log), 1, "撤销停的不叫醒她");
    held.end(Exit::Code(0));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(
        job_reported(&home.log(handle.id())).len(),
        1,
        "停了以后它自己退出，不再报"
    );

    // 恢复撤销：停掉的不再起来，也不再停。
    assert!(matches!(
        ask(&handle, "cmd-3", Command::Unrevert).await,
        Ok(Outcome::Accepted { .. })
    ));
    assert_eq!((held.killed(), table.stopped().len()), (1, 1));
    let log = home.log(handle.id());
    assert_eq!(
        (
            job_reported(&log).len(),
            child_reported(&log).len(),
            turns(&log)
        ),
        (1, 1, 1),
        "undone 的照留，不开轮"
    );
}

#[tokio::test]
async fn a_command_that_already_ended_is_not_killed_again() {
    let home = Home::new();
    let held = Held::new(&[]);
    let table = Arc::new(Table::default());
    let script = Script::new([
        Play::calls(&[("start", "{}")]),
        Play::Says("放出去了。"),
        Play::Says("跑完了。"),
    ]);
    let handle = session(&home, &script, &held, &table).await;
    talk(&handle, "cmd-1", "后台跑").await;
    let mut pushes = watch(&handle).await;
    held.end(Exit::Code(0));
    until_turn_ends(&mut pushes).await;
    assert!(matches!(
        ask(&handle, "cmd-2", Command::Revert { turn: None }).await,
        Ok(Outcome::Accepted { .. })
    ));
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    let log = home.log(handle.id());
    assert_eq!(held.killed(), 0, "结束了的不杀");
    let [(_, reported)] = job_reported(&log)[..] else {
        panic!("只有它自己退出的那一条：{:?}", kinds(&log))
    };
    assert_eq!(reported.reason, JobReason::Exited);
}

//! 发回话的任务结束了（`onebot.md`「施工时定的」第 14 条）：崩了照实交上去，桥停下；好好结束的接着办。

use tokio::task::JoinSet;

use super::sent;
use crate::serve::Failure;

#[tokio::test]
async fn a_crashed_reply_task_stops_the_bridge() {
    let mut tasks = JoinSet::new();
    tasks.spawn(async { panic!("测试里故意崩") });
    let joined = tasks.join_next().await.expect("有一个");
    match sent(joined) {
        Err(Failure::Crashed(reason)) => assert!(reason.contains("panic"), "{reason}"),
        other => panic!("崩了要照实交上去：{other:?}"),
    }
}

#[tokio::test]
async fn a_reply_task_that_ended_well_goes_on() {
    let mut tasks = JoinSet::new();
    tasks.spawn(async {});
    let joined = tasks.join_next().await.expect("有一个");
    assert_eq!(sent(joined), Ok(()));
}

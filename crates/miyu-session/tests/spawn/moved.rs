//! 任务表变了报一声（施工 9-8 补下修）：派出子代理的那一批，经会话表的端口报 `moved`，核心的会话树照它重量。只靠开轮、空下来
//! 重量的话，孙代理开轮那一声可能先于子代理记下它的任务，会话树少数一个（CI run 1612 的偶发红）。

use super::*;

#[tokio::test]
async fn starting_a_child_tells_the_table_the_jobs_moved() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        calls(&[subagent("查", "Look around.")]),
        Play::Says("派出去了。"),
    ]);
    let handle = parent(&home, &script, &table).await;
    assert!(table.moved().is_empty(), "还没派");
    one_turn(&home, &handle, 1).await;
    assert!(
        table.moved().contains(handle.id()),
        "派出去的那一批报了任务表变了：{:?}",
        table.moved()
    );
}

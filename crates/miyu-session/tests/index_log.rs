//! 会话列表的索引更新失败不影响会话（施工 3-8 七补，`session/actor.md` 第 5 条第 7 点）：索引里的表没了，落盘照样成，一轮照常
//! 说完；记一行带会话编号的 `WARN session index not updated`。索引好好的时候，每落一批，那一行照到日志的末尾。
//!
//! 全局装一个写进内存的订阅者：更新索引在写盘的阻塞线程里。全局的一个进程只能装一次，所以这个文件单独一个测试程序，
//! 只有一个测试。

mod support;

use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use miyu_store::index::FILE;

use support::*;

#[tokio::test]
async fn a_failed_index_update_leaves_the_session_running() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("还在。")]);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("第一句"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let rows = home.index.rows().expect("读得了");
    let row = &rows[handle.id()];
    assert_eq!(
        row.mark.next.get(),
        home.log(handle.id()).len() as u64 + 1,
        "每一批都盖上了"
    );
    assert!(
        memory
            .lines()
            .iter()
            .all(|line| !line.contains("session index"))
    );

    let path = home.root.index(&alice_account()).join(FILE);
    rusqlite::Connection::open(path)
        .expect("打得开")
        .execute_batch("DROP TABLE sessions")
        .expect("删得掉");
    ask(&handle, "cmd-2", say("第二句"))
        .await
        .expect("会话还在跑");
    until_turn_ends(&mut pushes).await;
    let said = home
        .log(handle.id())
        .iter()
        .filter(|event| matches!(event.body, miyu_kernel::event::Body::TurnEnded(_)))
        .count();
    assert_eq!(said, 2, "第二轮照常说完、落了盘");
    let lines = memory.lines();
    let line = lines
        .iter()
        .find(|line| line.contains("session index not updated"))
        .unwrap_or_else(|| panic!("记了更新失败：{lines:?}"));
    let expected = format!("WARN  session  {} session index not updated", handle.id());
    assert!(line.contains(&expected), "{line}");
}

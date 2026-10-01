//! 会话列表的索引记的运行日志（施工 3-8 七补，`protocol.md`「运行日志」）：新建的记 `INFO session index created`；起来时读不懂
//! 的删掉重建，记 `WARN session index rebuilt`；列会话时读出坏了的，记 `WARN session index not read`，删掉重建，这一次照样
//! 列得对。
//!
//! 全局装一个写进内存的订阅者：列会话在阻塞线程里。全局的一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use serde_json::json;

use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::Script;
use miyu_store::index::FILE;

use support::*;

#[tokio::test]
async fn created_rebuilt_and_found_broken_are_logged() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let has = |what: &str| memory.lines().iter().any(|line| line.contains(what));

    let fresh = Home::new();
    drop(fresh.core(&Script::new([])));
    assert!(
        has("INFO  endpoint session index created"),
        "{:?}",
        memory.lines()
    );

    let home = Home::new();
    let path = home.root.index(&alice()).join(FILE);
    std::fs::create_dir_all(path.parent().expect("有上一层")).expect("建得了");
    std::fs::write(&path, vec![b'x'; 4096]).expect("写得进");
    let mut client = Client::connect(home.core(&Script::new([])));
    assert!(
        has("WARN  endpoint session index rebuilt reason="),
        "{:?}",
        memory.lines()
    );
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    client.create("c1", &work).await;
    client.create("c2", &work).await;
    let before = client.call("l1", "session.list", json!({})).await;
    rusqlite::Connection::open(&path)
        .expect("打得开")
        .execute("UPDATE sessions SET owner = 'Not An Account'", [])
        .expect("改得了");
    let after = client.call("l2", "session.list", json!({})).await;
    assert_eq!(after["result"], before["result"], "照日志整份读，列得对");
    assert!(
        has("WARN  endpoint session index not read"),
        "{:?}",
        memory.lines()
    );
    let again = client.call("l3", "session.list", json!({})).await;
    assert_eq!(again["result"], before["result"]);
}

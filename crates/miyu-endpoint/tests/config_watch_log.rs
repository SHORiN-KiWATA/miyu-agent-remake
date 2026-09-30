//! 监视记的运行日志（施工 8-4，`docs/blueprint/config.md`「出错」）：手改被看到的记 `INFO config changed`（`via=file`，
//! 只有键名）；监视起不来记 `WARN config watch unavailable`，退回轮询，照旧看得到手改（Linux 上拿掉家目录让 inotify
//! 拒绝）。
//!
//! 全局装一个写进内存的订阅者：监视在它自己的线程上。全局的一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use std::sync::Arc;

use serde_json::json;

use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::Script;

use support::*;

/// 连上、握手、订阅配置的推送，改一份系统配置，等推来。
async fn edit_and_wait(home: &Home, core: &Arc<miyu_endpoint::Core>) {
    let mut client = Client::connect(Arc::clone(core));
    client.hello().await;
    client
        .call("s1", "subscribe", json!({"stream": "config"}))
        .await;
    home.write("system/config.toml", "[log]\nlevel = \"debug\"\n");
    let next = client.next().await.expect("推了");
    assert_eq!(next["params"]["via"], "file", "{next}");
}

#[tokio::test]
async fn a_hand_edit_and_an_unavailable_watch_are_logged() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let has = |what: &str| memory.lines().iter().any(|line| line.contains(what));

    let home = Home::new();
    home.root.prepare_home(&alice()).expect("建得了家目录");
    let core = home.core_configured(&Script::new([]), None, &[]);
    let watching = core.watch_config().expect("监视得了");
    assert_eq!(watching.unavailable(), None);
    edit_and_wait(&home, &core).await;
    assert!(
        has("INFO  config   config changed layer=system via=file keys=log.level"),
        "{:?}",
        memory.lines()
    );
    assert!(!has("config watch unavailable"), "{:?}", memory.lines());

    if cfg!(target_os = "linux") {
        let home = Home::new();
        let core = home.core_configured(&Script::new([]), None, &[]);
        // 家目录拿掉（会话列表的索引在里面，开着的照样开着）：看不了它，inotify 拒绝。
        std::fs::remove_dir_all(home.root.account_dir(&alice())).expect("删得掉");
        let watching = core.watch_config().expect("退回轮询，照样监视");
        assert!(watching.unavailable().is_some());
        assert!(
            has("WARN  config   config watch unavailable error="),
            "{:?}",
            memory.lines()
        );
        edit_and_wait(&home, &core).await;
    }
}

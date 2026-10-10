//! 陌生人的私聊只记一行运行日志（`onebot.md` 第一条「怎么走」第 7 条、「施工时定的」第 49 条）：核心 O-4 中以后，`venue.session`
//! 回的会话属主是桥自己（系统账号）的是陌生人，同一个人连发几条，运行日志只记一行 `not admin or whitelisted, not taken`。测试那一头照那时
//! 的样子改写账号。
//!
//! 全局装一个写进内存的订阅者，一个进程只能装一次，所以这个文件单独一个测试程序，只有一个测试。

mod support;

use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::Script;

use support::*;

#[tokio::test]
async fn a_stranger_is_logged_once() {
    let memory = Memory::new();
    tracing::subscriber::set_global_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ))
    .expect("这个测试程序只装这一次");
    let home = Home::new(&Script::new([]));
    let accounts = Accounts {
        own: "onebot",
        venue: Some("onebot"),
    };
    let (serve, relay) = serve_relayed(home.root.clone(), settings(), Some(accounts));
    let bridge = start(serve).await;
    let mut napcat = admin_napcat(bridge.port).await;
    for message in 1..=4 {
        napcat.admin_says(message, "在吗").await;
    }
    // 一条条照先后办：第四条去问 `venue.session` 的时候，前三条已经办完了。
    within("四条都问过会话", async {
        while relay
            .asked()
            .iter()
            .filter(|method| *method == "venue.session")
            .count()
            < 4
        {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    let refused: Vec<String> = memory
        .lines()
        .into_iter()
        .filter(|line| line.contains("not admin or whitelisted, not taken"))
        .collect();
    assert_eq!(refused.len(), 1, "同一个人只记一行：{refused:?}");
    assert!(refused[0].contains("venue=qq:private:10001"), "{refused:?}");
    bridge.stop().await.expect("停得下");
}

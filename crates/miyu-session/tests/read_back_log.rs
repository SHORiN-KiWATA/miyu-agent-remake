//! 撤掉压缩时日志读不回来（施工 6-9，`docs/blueprint/session/actor.md` 第 4 条）：会话停下，这个撤销收到「会话停了」，
//! 运行日志记一行 `read back failed, stopped`，和写不进去一样。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能
//! 漏听。

mod support;

use miyu_kernel::session::Command;
use miyu_log::{LevelFilter, Memory};
use miyu_session::Stopped;
use miyu_session::testkit::{Play, Script};
use support::{Home, alice_account, ask, say, until_turn_ends, watch};

#[tokio::test]
async fn a_log_that_cannot_be_read_back_stops_the_session() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    // 要第二轮里当场压出一个检查点：关掉提前压（施工 6-11 补起开着的会接着说，压缩挪到下一步）。
    let mut home = Home::new();
    home.configs = support::routing::configs("[compaction]\nprepare = false\n", &[]);
    // 窗口 33100，压缩线 100：剧本报 110，第二轮一开头就压（`tests/limits.rs`）。
    let script = Script::new([
        Play::Says("你好。"),
        Play::Says("<summary>用户打了招呼。</summary>"),
        Play::Says("好的。"),
    ])
    .window(33_100);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    ask(&handle, "cmd-2", say("再说一句"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    // 日志中间坏了一行：字节数不变，会话照旧往后写，读回时读不出来。
    let dir = home.root.session_dir(&alice_account(), handle.id());
    let segment = dir.join("000000000001.jsonl");
    let text = std::fs::read_to_string(&segment).expect("读得出");
    let second = text.lines().nth(1).expect("有第二行");
    let broken = text.replacen(second, &"x".repeat(second.len()), 1);
    std::fs::write(&segment, broken).expect("写得进");
    // 撤掉压缩所在的第二轮：要读回，读不回来，会话停下。
    let undone = ask(&handle, "cmd-3", Command::Revert { turn: None }).await;
    assert_eq!(undone, Err(Stopped), "撤销收到「会话停了」");
    let s = handle.id().as_str().to_string();
    let lines = memory.lines();
    let head = format!(" WARN  session  {s} read back failed, stopped error=");
    assert!(lines.iter().any(|line| line.contains(&head)), "{lines:#?}");
}

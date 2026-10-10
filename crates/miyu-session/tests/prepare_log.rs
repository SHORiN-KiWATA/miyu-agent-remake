//! 提前压好、到线换上（施工 6-11 上，`docs/blueprint/compaction.md` 第十五条、第十三条），真的会话：配置没写（默认开），
//! 过了起压线旁路发一次摘要请求，到线直接换上，不再请求；日志 `compacted` 那一行带 `prepared=yes`。提前压出错的记一行
//! `WARN compaction failed`。
//!
//! 只有这一个测试，自己一个进程（同 `compaction_log.rs`）。

mod support;

use miyu_kernel::event::ErrorClass;
use miyu_kernel::event::{Body, Purpose};
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use support::{Home, ask, say, stop, until_logged, until_turn_ends, watch};

/// 记下的提前压那几次。
fn prepared(log: &[miyu_kernel::event::Event]) -> usize {
    log.iter()
        .filter(|event| {
            matches!(&event.body, Body::ModelCalled(called)
                if called.purpose == Some(Purpose::Compaction) && event.turn.is_none())
        })
        .count()
}

#[tokio::test]
async fn a_prepared_summary_is_swapped_in_without_asking_again() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    let home = Home::new();
    // 窗口 105263：压缩线 80000（减掉预留 20000、窗口的 5% 5263），尾巴的预算 16000，提前量 4000，过了 76000 起压。剧本每次报 77000：第一轮整份照本地估、
    // 不到起压线；第二轮过了起压线，主请求、提前压各一次；第三轮说一大段（约 5000 token）过了线，换上。
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("嗯。"),
        Play::Says("<summary>提前压好的摘要</summary>"),
        Play::Says("行。"),
    ])
    .window(105_263)
    .reports(77_000);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say(&"a".repeat(70_000)))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    ask(&handle, "cmd-2", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    until_logged(&home, handle.id(), |log| prepared(log) == 1).await;
    ask(&handle, "cmd-3", say(&"b".repeat(20_000)))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&handle).await;
    assert_eq!(script.requests().len(), 4, "到线时没再请求");
    let log = home.log(handle.id());
    let summaries: Vec<&str> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted.summary.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(summaries, ["提前压好的摘要"]);
    let lines = memory.lines();
    let line = lines
        .iter()
        .find(|line| line.contains(" compacted "))
        .unwrap_or_else(|| panic!("没有压好了那一行：{lines:#?}"));
    assert!(line.contains(" trigger=auto "), "{line}");
    assert!(line.ends_with(" prepared=yes"), "{line}");
    assert!(
        lines
            .iter()
            .any(|line| line.contains(" compaction request")),
        "旁路请求那一行：{lines:#?}"
    );
    // 另一个会话：提前压那一次出错，记一行 WARN。
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("嗯。"),
        Play::Fails {
            class: ErrorClass::Retryable,
            wait_ms: None,
        },
    ])
    .window(105_263)
    .reports(77_000);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    for (id, words) in [("cmd-1", "a".repeat(70_000)), ("cmd-2", "hi".to_string())] {
        ask(&handle, id, say(&words)).await.expect("会话在跑");
        until_turn_ends(&mut pushes).await;
    }
    until_logged(&home, handle.id(), |log| prepared(log) == 1).await;
    stop(&handle).await;
    let failed = format!("WARN  session  {} compaction failed ", handle.id().as_str());
    let lines = memory.lines();
    assert!(
        lines.iter().any(|line| line.contains(&failed)),
        "{lines:#?}"
    );
}

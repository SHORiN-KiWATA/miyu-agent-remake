//! 手动压缩压好了也记一行度量（施工 6-8，`docs/blueprint/compaction.md` 第十三条）：`trigger` 照 `compaction.done` 写
//! `manual`，不是写死的 `auto`。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能
//! 漏听（和 `compaction_log.rs` 一样）。

mod support;

use miyu_kernel::session::Command;
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use support::{Home, ask, say, stop, until_turn_ends, watch};

#[tokio::test]
async fn a_manual_compaction_writes_its_line_as_manual() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    let home = Home::new();
    // 窗口大到不会自动压。出厂的尾巴是 16000 token：第一句说七万个字，一组就超了尾巴，压得掉它。
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("<summary>用户说了很长的一段。</summary>"),
    ])
    .window(1_000_000);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say(&"x".repeat(70_000)))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let compact = Command::Compact {
        instructions: Some("keep the plan".to_string()),
    };
    ask(&handle, "cmd-2", compact).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&handle).await;
    let s = handle.id().as_str().to_string();
    let seen = script.requests()[1].0.get();
    let lines = memory.lines();
    let line = lines
        .iter()
        .find(|line| line.contains(" compacted "))
        .unwrap_or_else(|| panic!("没有压好了那一行：{lines:#?}"));
    let rest = line.splitn(3, ' ').nth(2).unwrap_or_default();
    let head = format!("INFO  session  {s} compacted seen={seen} trigger=manual before=");
    assert!(rest.starts_with(&head), "{rest}");
    // 人附的要求、对话的字一个都不写。
    assert!(!rest.contains("keep the plan"), "{rest}");
    assert!(!rest.contains("很长"), "{rest}");
}

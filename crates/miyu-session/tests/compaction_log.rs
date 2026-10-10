//! 压好了记一行度量（施工 6-3 下，`docs/blueprint/compaction.md` 第十三条）：为什么压、压前、压后，摘要请求的输入、
//! 命中、输出、用时，写在会话编号后面。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能
//! 漏听。

mod support;

use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use support::{Home, ask, say, stop, until_turn_ends, watch};

#[tokio::test]
async fn a_compaction_writes_one_line_of_numbers() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    // 测的是到线当场压的那一行：关掉提前压（施工 6-11 补起开着的会接着说，压缩挪到下一步）。
    let mut home = Home::new();
    home.configs = support::routing::configs("[compaction]\nprepare = false\n", &[]);
    // 窗口 21157，压缩线 100（减掉预留 20000、窗口的 5% 1057）：剧本报 110，第二轮一开头就压（`tests/limits.rs`）。
    let script = Script::new([
        Play::Says("你好。"),
        Play::Says("<summary>用户打了招呼。</summary>"),
        Play::Says("好的。"),
    ])
    .window(21_157);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    ask(&handle, "cmd-2", say("再说一句"))
        .await
        .expect("会话在跑");
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
    let head = format!("INFO  session  {s} compacted seen={seen} trigger=auto before=");
    assert!(rest.starts_with(&head), "{rest}");
    assert!(
        rest.contains(" summary_in=100 summary_cached=40 summary_out=10 took_ms="),
        "{rest}"
    );
    // 对话的字一个都不写。
    assert!(!rest.contains("招呼"), "{rest}");
}

//! 模型的限额（施工 6-3 上）：会话 actor 造会话、载入以后先把端口交的限额交给内核，到线就在这一轮里压。
//!
//! 剧本的回复报 110（60 没命中、40 命中、10 输出）；窗口 21157，没报最大输出，压缩线 = 21157 − 20000 − 1057（余量至多窗口的 5%）= 100：
//! 第一轮没有锚、请求很短，不压；第二轮的锚是 110，一开头就过线。
//!
//! 给头看的那一份（施工 6-3 补）：`Handle` 带着端口交的窗口和内核算的压缩线，造会话、载入的都一样。

use crate::support::{Home, ask, say, stop, until_turn_ends, watch};
use miyu_kernel::block::Block;
use miyu_kernel::event::Body;
use miyu_kernel::request::Message;
use miyu_kernel::session::ContextLimits;
use miyu_session::testkit::{Play, Script};

/// 窗口 21157 的剧本：第一轮答一句；第二轮先回摘要请求，再答一句。
fn script() -> Script {
    Script::new([
        Play::Says("你好。"),
        Play::Says("<analysis>a</analysis><summary>用户打了招呼。</summary>"),
        Play::Says("好的。"),
    ])
    .window(21_157)
}

/// 这一次请求是不是摘要请求：最后一块是出厂的摘要指令。
fn is_summary(request: &miyu_kernel::request::Request) -> bool {
    matches!(
        request.messages.last(),
        Some(Message::User { blocks })
            if matches!(blocks.last(), Some(Block::Text(text)) if text.text.starts_with("Respond with text only."))
    )
}

#[tokio::test]
async fn a_new_session_knows_the_window_and_compacts_at_the_line() {
    // 测的是到线当场压：关掉提前压（施工 6-11 补起开着的会接着说，压缩挪到下一步，见内核的 `scenario/prepare_go.rs`）。
    let mut home = Home::new();
    home.configs = crate::support::routing::configs("[compaction]\nprepare = false\n", &[]);
    let script = script();
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    ask(&handle, "cmd-2", say("再说一句"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let requests = script.requests();
    let summaries: Vec<bool> = requests
        .iter()
        .map(|(_, request)| is_summary(request))
        .collect();
    assert_eq!(summaries, [false, true, false]);
    let log = home.log(handle.id());
    assert!(
        log.iter()
            .any(|event| matches!(event.body, Body::ContextCompacted(_))),
        "写了压缩"
    );
    stop(&handle).await;
}

#[tokio::test]
async fn a_loaded_session_knows_the_window_too() {
    // 测的是到线当场压：关掉提前压（施工 6-11 补起开着的会接着说，压缩挪到下一步，见内核的 `scenario/prepare_go.rs`）。
    let mut home = Home::new();
    home.configs = crate::support::routing::configs("[compaction]\nprepare = false\n", &[]);
    let script = script();
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let session = handle.id().clone();
    stop(&handle).await;
    // 载入以后，限额照样先交给内核：第二轮一开头就压。
    let handle = home.load(&session, &script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-2", say("再说一句"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let summaries: Vec<bool> = script
        .requests()
        .iter()
        .map(|(_, request)| is_summary(request))
        .collect();
    assert_eq!(summaries, [false, true, false]);
    stop(&handle).await;
}

#[tokio::test]
async fn without_a_window_it_is_never_compacted() {
    let home = Home::new();
    let script = Script::new([Play::Says("你好。"), Play::Says("好的。")]);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    ask(&handle, "cmd-2", say("再说一句"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert_eq!(script.requests().len(), 2);
    stop(&handle).await;
}

#[tokio::test]
async fn the_handle_carries_the_window_and_the_line_the_kernel_uses() {
    let home = Home::new();
    let script = script();
    let expected = ContextLimits {
        window: Some(21_157),
        compaction_line: Some(100),
    };
    let handle = home.create(&script).await;
    assert_eq!(handle.limits(), expected, "造会话的");
    let session = handle.id().clone();
    stop(&handle).await;
    let handle = home.load(&session, &script).await;
    assert_eq!(handle.limits(), expected, "载入的");
    stop(&handle).await;
}

#[tokio::test]
async fn without_a_window_the_handle_carries_neither() {
    let home = Home::new();
    let script = Script::new([]);
    let nothing = ContextLimits {
        window: None,
        compaction_line: None,
    };
    let handle = home.create(&script).await;
    assert_eq!(handle.limits(), nothing, "造会话的");
    let session = handle.id().clone();
    stop(&handle).await;
    let handle = home.load(&session, &script).await;
    assert_eq!(handle.limits(), nothing, "载入的");
    stop(&handle).await;
}

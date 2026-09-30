//! 回顾的请求也记收场的那几行（施工 3-8 四补，`docs/blueprint/session/actor.md` 第 7 条）：交给端口前一行 `recap request`，
//! 说完了一行 `recap ended`，没写成的一行 `recap failed`，名字都是它照到的那一条；和主请求的几行一样，不写对话的字。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能漏听
//! （和 `log.rs` 一样）。

mod support;

use miyu_kernel::event::ErrorClass;
use miyu_kernel::session::{Command, Outcome};
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use support::{Home, ask, say, stop, until_turn_ends, watch};

/// 一行去掉时刻，用时换成 `_`。
fn shape(line: &str) -> String {
    let rest = line.splitn(3, ' ').nth(2).unwrap_or_default();
    match rest.split_once(" took_ms=") {
        Some((head, tail)) => {
            let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
            format!("{head} took_ms=_{}", &tail[digits..])
        }
        None => rest.to_string(),
    }
}

#[tokio::test]
async fn a_recap_writes_its_request_and_ending_lines() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    let home = Home::new();
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("在打招呼。"),
        Play::Says("嗯。"),
        Play::Fails {
            class: ErrorClass::Unclassified,
            wait_ms: None,
        },
    ]);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("早上好"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let done = ask(&handle, "cmd-2", Command::Recap).await;
    assert!(matches!(done, Ok(Outcome::Recapped { .. })), "{done:?}");
    ask(&handle, "cmd-3", say("再说一句"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let failed = ask(&handle, "cmd-4", Command::Recap).await;
    assert!(matches!(failed, Ok(Outcome::Rejected { .. })), "{failed:?}");
    stop(&handle).await;
    let s = handle.id().as_str().to_string();
    let recaps: Vec<String> = memory
        .lines()
        .iter()
        .map(|line| shape(line))
        .filter(|line| line.contains(" recap "))
        .collect();
    let (first, second) = (script.requests()[1].0.get(), script.requests()[3].0.get());
    assert_eq!(
        recaps,
        [
            format!(
                "INFO  session  {s} recap request seen={first} endpoint=deepseek model=deepseek-v4"
            ),
            format!("INFO  session  {s} recap ended seen={first} took_ms=_ in=100 hit=40 out=10"),
            format!(
                "INFO  session  {s} recap request seen={second} endpoint=deepseek model=deepseek-v4"
            ),
            format!("INFO  session  {s} recap failed seen={second} took_ms=_ class=other"),
        ]
    );
    assert!(
        memory.lines().iter().all(|line| !line.contains("打招呼")),
        "不写对话的字"
    );
}

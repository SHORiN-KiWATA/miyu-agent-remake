//! 执行工具的运行日志（施工 4-2，`28-运行日志.md`）：开始跑、跑完、出错、叫停、崩了、目录里没有，各记
//! 一行；参数的字一个都不记。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，
//! 这里装的订阅者可能漏听。

mod support;

use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::session::{Command, Queued};
use miyu_kernel::tool::Access;
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};
use support::{Home, ask, say, stop, until_turn_ends, watch, within};

/// 一行去掉时刻和会话编号，用时换成 `_`，调用编号里的序号换成 `N`：这几样每次不一样。
fn shape(line: &str) -> String {
    let mut words: Vec<String> = line.split(' ').skip(2).map(str::to_string).collect();
    words.retain(|word| !word.is_empty());
    words
        .into_iter()
        .enumerate()
        .filter(|(i, _)| *i != 2)
        .map(|(_, word)| {
            if let Some(ms) = word.strip_prefix("took_ms=") {
                assert!(ms.bytes().all(|b| b.is_ascii_digit()), "{line}");
                "took_ms=_".to_string()
            } else if let Some(call) = word.strip_prefix("call=call_") {
                let (_, index) = call.split_once('_').expect("call_<序号>_<第几个>");
                format!("call=call_N_{index}")
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// 等到 `ready` 为真，最多十秒。
async fn until(what: &str, mut ready: impl FnMut() -> bool) {
    within(what, async {
        while !ready() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
}

#[tokio::test]
async fn the_log_says_which_tools_ran_and_nothing_that_was_passed() {
    let memory = Memory::new();
    let _listening =
        tracing::subscriber::set_default(miyu_log::subscriber(memory.clone(), LevelFilter::INFO));
    let home = Home::new();
    let echo = Fake::new("echo", Access::Read, Act::Echo);
    let fail = Fake::new("fail", Access::Read, Act::Fails("紫色的错"));
    let boom = Fake::new("boom", Access::Read, Act::Panics);
    let wait = Fake::new("wait", Access::Read, Act::Holds);
    let tools =
        Catalog::new([&echo, &fail, &boom, &wait].map(|tool| Arc::clone(tool) as Arc<dyn Tool>))
            .expect("都合写法");
    let script = Script::new([
        Play::Calls(&[("echo", r#"{"note":"紫色的猫"}"#), ("fail", "{}")]),
        Play::Says("好。"),
        Play::Calls(&[("boom", "{}")]),
        Play::Says("好。"),
        Play::Calls(&[("wait", "{}")]),
    ]);
    let handle = home.create_with(&script, &tools).await;
    let session = handle.id().clone();
    let mut pushes = watch(&handle).await;
    for command in ["cmd-1", "cmd-2"] {
        ask(&handle, command, say("hi")).await.expect("会话在跑");
        until_turn_ends(&mut pushes).await;
    }
    ask(&handle, "cmd-3", say("hi")).await.expect("会话在跑");
    until("wait 开始跑", || wait.calls().len() == 1).await;
    let interrupt = Command::Interrupt {
        queued: Queued::Return,
    };
    ask(&handle, "cmd-4", interrupt).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&handle).await;
    // 核心升级拿掉了 echo：载入的老会话照样调它。
    let again = Script::new([Play::Calls(&[("echo", "{}")]), Play::Says("好。")]);
    let loaded = home.load_with(&session, &again, &Catalog::default()).await;
    let mut pushes = watch(&loaded).await;
    ask(&loaded, "cmd-5", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;

    let lines = memory.lines();
    let mut tools: Vec<String> = lines
        .iter()
        .filter(|line| line.contains(session.as_str()) && line.contains(" call="))
        .map(|line| shape(line))
        .collect();
    // 一起跑的两件，谁先跑完不一定：排一下再比。
    tools[2..4].sort();
    assert_eq!(
        tools,
        [
            "INFO session running call=call_N_1 tool=echo",
            "INFO session running call=call_N_2 tool=fail",
            "INFO session ran call=call_N_1 took_ms=_",
            "INFO session ran call=call_N_2 took_ms=_ error=true",
            "INFO session running call=call_N_1 tool=boom",
            "ERROR session crashed call=call_N_1 tool=boom took_ms=_",
            "INFO session running call=call_N_1 tool=wait",
            "INFO session stopped call=call_N_1 took_ms=_",
            "WARN session unavailable call=call_N_1 tool=echo",
        ],
        "{lines:#?}"
    );
    // 参数、工具交回的字，一个都不在日志里。
    for line in &lines {
        for said in ["紫色的猫", "紫色的错", "cwd="] {
            assert!(!line.contains(said), "日志里出现了「{said}」：{line}");
        }
    }
}

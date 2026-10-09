//! 判官带的人格（施工 O-23 补，`onebot.md` 第一条「群里怎么叫她」第 12 条第 3 款）：核心那一头是测试，钟停住。有人格的拿到记录
//! 以后读 `persona.read`，原文夹在人格的两个标签中间、紧跟着 `system.txt`；读到的记 60 秒，到了再读，别的人格另读；没写人设的
//! 不带、照样记下；读不到的不带、照样问、不记下；没给人格的不读（`tests.rs` 头一个测试）；读的时候核心断开了交回空的。夹具在
//! `tests.rs`。

use std::time::Duration;

use serde_json::{Value, json};

use super::Asking;
use super::tests::{asked, asked_with, asking, connected, kept, slots, verdict};

/// 群会话用人格 `persona` 的那一问。
fn with_persona(persona: &str) -> Asking {
    asking(|asking| asking.persona = Some(persona.to_string()))
}

/// 判官看到的 system。
fn system_of(call: &Value) -> String {
    call["params"]["messages"][0]["text"]
        .as_str()
        .expect("是字")
        .to_string()
}

#[tokio::test(start_paused = true)]
async fn the_persona_is_read_after_the_records_and_comes_right_after_the_system_text() {
    let (caller, mut core) = connected();
    let task = asked(with_persona("engineer"), &caller, &slots());
    core.records("R1\n", "C1\n").await;
    let read = core.persona(json!("You are an engineer.")).await;
    assert_eq!(
        read["params"],
        json!({"persona": "engineer", "prompt": "persona"})
    );
    let call = core.says(&verdict("x")).await;
    let system = system_of(&call);
    assert!(
        system.starts_with("<system>\n<persona>\nYou are an engineer.\n</persona>\n<reply>\n"),
        "{system}"
    );
    let answer = task.await.expect("没崩").expect("核心在");
    assert_eq!(answer.result.expect("读得出").reason, "x");
}

#[tokio::test(start_paused = true)]
async fn a_persona_read_is_kept_for_a_while_then_read_again() {
    let (caller, mut core) = connected();
    let personas = kept();
    let first = asked_with(with_persona("engineer"), &caller, &slots(), &personas);
    core.records("", "C\n").await;
    core.persona(json!("Old words.")).await;
    core.says(&verdict("first")).await;
    first.await.expect("没崩").expect("核心在");
    // 59 秒以后：同一个人格不再读（`says` 只认 `model.call`），照记着的带。
    tokio::time::advance(Duration::from_secs(59)).await;
    let kept_one = asked_with(with_persona("engineer"), &caller, &slots(), &personas);
    core.records("", "C\n").await;
    let call = core.says(&verdict("kept")).await;
    assert!(system_of(&call).contains("Old words."), "记着的照带");
    kept_one.await.expect("没崩").expect("核心在");
    // 别的人格另读。
    let other = asked_with(with_persona("poet"), &caller, &slots(), &personas);
    core.records("", "C\n").await;
    let read = core.persona(json!("A poet.")).await;
    assert_eq!(read["params"]["persona"], "poet");
    let call = core.says(&verdict("other")).await;
    assert!(system_of(&call).contains("A poet.") && !system_of(&call).contains("Old words."));
    other.await.expect("没崩").expect("核心在");
    // 读到以后满 60 秒：再读，带新的。
    tokio::time::advance(Duration::from_secs(1)).await;
    let again = asked_with(with_persona("engineer"), &caller, &slots(), &personas);
    core.records("", "C\n").await;
    core.persona(json!("New words.")).await;
    let call = core.says(&verdict("again")).await;
    let system = system_of(&call);
    assert!(
        system.contains("New words.") && !system.contains("Old words."),
        "{system}"
    );
    again.await.expect("没崩").expect("核心在");
}

#[tokio::test(start_paused = true)]
async fn a_persona_without_words_is_left_out_and_still_kept() {
    let (caller, mut core) = connected();
    let personas = kept();
    // 没写人设的（`null`、空的字）不带；记下了，第二次不再读（`says` 只认 `model.call`）。
    for (persona, text) in [("blank", Value::Null), ("empty", json!(""))] {
        for first in [true, false] {
            let task = asked_with(with_persona(persona), &caller, &slots(), &personas);
            core.records("", "C\n").await;
            if first {
                core.persona(text.clone()).await;
            }
            let call = core.says(&verdict("x")).await;
            let system = system_of(&call);
            assert!(!system.contains("<persona>"), "{persona}：{system}");
            task.await.expect("没崩").expect("核心在");
        }
    }
}

#[tokio::test(start_paused = true)]
async fn an_unreadable_persona_is_left_out_and_read_again_next_time() {
    let (caller, mut core) = connected();
    let personas = kept();
    for _ in 0..2 {
        let task = asked_with(with_persona("gone"), &caller, &slots(), &personas);
        core.records("", "C\n").await;
        let read = core.next().await;
        assert_eq!(read["method"], "persona.read", "读不到的不记下，每次都读");
        core.refuse(&read, "unknown_persona");
        let call = core.says(&verdict("asked anyway")).await;
        let system = system_of(&call);
        assert!(!system.contains("<persona>"), "读不到的不带：{system}");
        let answer = task.await.expect("没崩").expect("核心在");
        assert_eq!(answer.result.expect("照样问了").reason, "asked anyway");
    }
}

#[tokio::test(start_paused = true)]
async fn a_core_gone_while_reading_the_persona_gives_nothing() {
    let (caller, mut core) = connected();
    let task = asked(with_persona("engineer"), &caller, &slots());
    core.records("", "C\n").await;
    let read = core.next().await;
    assert_eq!(read["method"], "persona.read");
    core.waiting.close();
    assert_eq!(task.await.expect("没崩"), None);
}

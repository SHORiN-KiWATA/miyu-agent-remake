//! 起标题的请求也记收场的那几行（施工 3-8 五补，`docs/blueprint/session/actor.md` 第 7 条）：交给端口前一行 `title request`，
//! 说完了一行 `title ended`，没起成的一行 `title failed`，名字都是它照到的那一条；两次都没起成就不再试，第二行
//! `title failed` 就是那一行。和主请求的几行一样，不写对话的字。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能漏听
//! （和 `log.rs` 一样）。

mod support;

use miyu_kernel::event::{Body, ErrorClass, Purpose};
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Pushed, Subscription};
use support::{Home, ask, say, stop, until_turn_ends, watch, within};

/// 等到推来起标题的那一条 `model.called`：收场的那一行已经记了，再停下会话。
async fn until_titled(subscription: &mut Subscription) {
    loop {
        let next = within("推送", subscription.next()).await.expect("订阅没断");
        if let Pushed::Events(events) = &*next
            && events.iter().any(|event| {
                matches!(&event.body, Body::ModelCalled(called) if called.purpose == Some(Purpose::Title))
            })
        {
            return;
        }
    }
}

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

/// 起标题的那几行：失败两次就不再试；起成了的一行 `title ended`。
#[tokio::test]
async fn titles_write_their_request_and_ending_lines() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::INFO,
        None,
    ));
    let home = Home::new();
    let failing = || Play::Fails {
        class: ErrorClass::Unclassified,
        wait_ms: None,
    };
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。"), Play::Says("行。")])
        .titles([failing(), failing()]);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    for (id, words) in [("cmd-1", "早上好"), ("cmd-2", "再说一句")] {
        ask(&handle, id, say(words)).await.expect("会话在跑");
        until_turn_ends(&mut pushes).await;
        until_titled(&mut pushes).await;
    }
    ask(&handle, "cmd-3", say("第三句"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&handle).await;
    let s = handle.id().as_str().to_string();
    let titled = script.titled();
    assert_eq!(titled.len(), 2, "两次都没起成，第三轮不再试");
    let (first, second) = (titled[0].0.get(), titled[1].0.get());
    let lines: Vec<String> = memory
        .lines()
        .iter()
        .map(|line| shape(line))
        .filter(|line| line.contains(" title "))
        .collect();
    let request = |seen: u64| {
        format!("INFO  session  {s} title request seen={seen} endpoint=deepseek model=deepseek-v4")
    };
    let failed =
        |seen: u64| format!("INFO  session  {s} title failed seen={seen} took_ms=_ class=other");
    assert_eq!(
        lines,
        [
            request(first),
            failed(first),
            request(second),
            failed(second)
        ]
    );

    // 起成了的：一行 `title ended`，用量照剧本的。
    let script = Script::new([Play::Says("好。")]).titles([Play::Says("打招呼")]);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("早上好"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    until_titled(&mut pushes).await;
    stop(&handle).await;
    let t = handle.id().as_str().to_string();
    let seen = script.titled()[0].0.get();
    let ended =
        format!("INFO  session  {t} title ended seen={seen} took_ms=_ in=100 hit=40 out=10");
    let found = memory
        .lines()
        .iter()
        .map(|line| shape(line))
        .any(|line| line == ended);
    assert!(found, "{:#?}", memory.lines());
    assert!(
        memory.lines().iter().all(|line| !line.contains("打招呼")),
        "不写对话的字"
    );
}

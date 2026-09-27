//! 会话的运行日志（`docs/construction/3-7-会话actor（中）.md`「做什么」的日志表）：造、请求、出错、重试、
//! 收场、停下、载入、没人拿着了、端口 panic 了；`DEBUG` 记每一条输入、每一个动作的种类。日志里没有
//! 对话的字。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，
//! 这里装的订阅者可能漏听。

mod support;

use std::time::Duration;

use miyu_kernel::event::{Body, ErrorClass};
use miyu_kernel::session::{Command, Outcome};
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Pushed, Stopped};
use support::{Home, ask, say, stop, until_turn_ends, watch};

/// 一行去掉时刻，用时换成 `_`：这两样每次不一样。
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

/// 等到日志里有一行以 `end` 结尾，最多五秒。
async fn wait_for(memory: &Memory, end: &str) {
    let found = tokio::time::timeout(Duration::from_secs(5), async {
        while !memory.lines().iter().any(|line| line.ends_with(end)) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(
        found.is_ok(),
        "五秒内没等到「{end}」：{:#?}",
        memory.lines()
    );
}

#[tokio::test]
async fn the_log_says_what_happened_and_nothing_that_was_said() {
    let memory = Memory::new();
    let listening =
        tracing::subscriber::set_default(miyu_log::subscriber(memory.clone(), LevelFilter::INFO));
    let home = Home::new();

    // 一个会话：先限速，重试以后说完；停下，载入，没人拿着了。
    let script = Script::new([
        Play::Fails {
            class: ErrorClass::RateLimited,
            wait_ms: Some(10),
        },
        Play::Says("你好。"),
    ]);
    let handle = home.create(&script).await;
    let session = handle.id().clone();
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("早上好"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&handle).await;
    let events = home.log(&session).len();
    let loaded = home.load(&session, &Script::new([])).await;
    drop((handle, loaded, pushes));
    let s = session.as_str();
    wait_for(&memory, &format!("session  {s} closed")).await;

    // 另一个会话：端口一叫就 panic。
    let broken = home.create(&Script::new([Play::Panics])).await;
    let b = broken.id().as_str().to_string();
    // 这句话落了盘就回应了，请求模型时端口才 panic。
    let said = ask(&broken, "cmd-1", say("早上好")).await;
    assert!(matches!(said, Ok(Outcome::Accepted { .. })), "{said:?}");
    wait_for(&memory, &format!("session  {b} panicked, stopped")).await;
    assert_eq!(
        ask(&broken, "cmd-2", say("早上好")).await,
        Err(Stopped),
        "panic 了的会话停了"
    );

    let requests = script.requests();
    let (first, second) = (requests[0].0.get(), requests[1].0.get());
    let lines = memory.lines();
    let mine: Vec<String> = lines
        .iter()
        .filter(|line| line.contains(s))
        .map(|line| shape(line))
        .collect();
    assert_eq!(
        mine,
        [
            format!("INFO  session  {s} created persona=engineer venue=local"),
            format!("INFO  session  {s} request seen={first} endpoint=deepseek model=deepseek-v4"),
            format!("INFO  session  {s} failed seen={first} took_ms=_ class=rate_limited"),
            format!(
                "WARN  session  {s} retrying seen={first} attempt=1 limit=5 wait_ms=10 class=rate_limited"
            ),
            format!("INFO  session  {s} request seen={second} endpoint=deepseek model=deepseek-v4"),
            format!("INFO  session  {s} ended seen={second} took_ms=_ in=100 hit=40 out=10"),
            format!("INFO  session  {s} stopped"),
            format!("INFO  session  {s} loaded events={events}"),
            format!("INFO  session  {s} closed"),
        ],
        "{lines:#?}"
    );
    let theirs: Vec<String> = lines
        .iter()
        .filter(|line| line.contains(&b))
        .map(|line| shape(line))
        .collect();
    assert_eq!(theirs.len(), 3, "{theirs:#?}");
    assert_eq!(
        theirs[0],
        format!("INFO  session  {b} created persona=engineer venue=local")
    );
    assert!(
        theirs[1].starts_with(&format!("INFO  session  {b} request seen=")),
        "{theirs:#?}"
    );
    assert_eq!(theirs[2], format!("ERROR session  {b} panicked, stopped"));
    // 对话的字一个都没有：人说的、她说的、供应商出错的原话。
    for line in &lines {
        for said in ["早上好", "你好", "slow down"] {
            assert!(!line.contains(said), "日志里有「{said}」：{line}");
        }
    }

    // 再一个会话：说完一轮，撤销它，再说一句。这一次的请求少了撤掉的那几条，前缀断开了：`request` 那一行
    // 写上第一处不同在哪（施工 3-9 下）。
    let redo = home
        .create(&Script::new([Play::Says("好。"), Play::Says("嗯。")]))
        .await;
    let r = redo.id().as_str().to_string();
    let mut pushes = watch(&redo).await;
    ask(&redo, "cmd-1", say("第一句")).await.expect("会话在跑");
    let turn = until_turn_ends(&mut pushes)
        .await
        .iter()
        .find_map(|pushed| match &**pushed {
            Pushed::Events(events) => events.iter().find_map(|event| match event.body {
                Body::TurnStarted(_) => event.turn,
                _ => None,
            }),
            Pushed::Transient(_) => None,
        })
        .expect("开过这一轮");
    ask(&redo, "cmd-2", Command::Revert { turn })
        .await
        .expect("会话在跑");
    ask(&redo, "cmd-3", say("重说")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let requests: Vec<String> = memory
        .lines()
        .iter()
        .filter(|line| line.contains(&r) && line.contains(" request "))
        .map(|line| shape(line))
        .collect();
    assert_eq!(requests.len(), 2, "{requests:#?}");
    assert!(
        requests[0].ends_with("model=deepseek-v4"),
        "第一次前面没有请求可比，不写：{requests:#?}"
    );
    assert!(
        requests[1].ends_with(" changed=message:0:user"),
        "撤销以后前缀从第 0 条（人说的那一句）断开：{requests:#?}"
    );
    drop((redo, pushes));

    // DEBUG：每一条输入、每一个动作的种类；增量在 TRACE，看不到。
    drop(listening);
    let memory = Memory::new();
    let _listening =
        tracing::subscriber::set_default(miyu_log::subscriber(memory.clone(), LevelFilter::DEBUG));
    let handle = home.create(&Script::new([Play::Says("你好。")])).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("早上好"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let lines: Vec<String> = memory.lines().iter().map(|line| shape(line)).collect();
    let d = handle.id().as_str();
    for kind in [
        "action kind=append",
        "input kind=stored",
        "action kind=push",
        "action kind=reply",
        "input kind=command",
        "action kind=call_model",
        "input kind=request_sent",
        "input kind=model_ended",
    ] {
        let line = format!("DEBUG session  {d} {kind}");
        assert!(lines.contains(&line), "{line} 应该在 {lines:#?} 里");
    }
    assert!(
        !lines.iter().any(|line| line.contains("kind=model_delta")),
        "增量在 TRACE：{lines:#?}"
    );
}

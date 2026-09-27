//! 会话 actor（`docs/construction/3-7-会话actor（中）.md` 验收第 2 条）：造会话、说一句、重试、打断、
//! 停下再载入、同一个命令来两次。请求模型照剧本回，磁盘是临时的数据根。

mod support;

use std::time::{Duration, Instant};

use miyu_kernel::event::{Body, EndReason, ErrorClass, Event, TransientBody};
use miyu_kernel::id::Seq;
use miyu_kernel::session::{Command, Outcome, Queued};
use miyu_kernel::time::Timestamp;
use miyu_policy::Snapshot;
use miyu_session::{Pushed, Stopped};
use miyu_store::blob::Blobs;
use support::{
    Home, Play, Script, alice_account, ask, id, kinds, say, stop, until_delta, until_logged,
    until_turn_ends, watch,
};

/// 一份推送里的事件的序号；瞬时事件没有。
fn seqs(pushed: &Pushed) -> Vec<Seq> {
    match pushed {
        Pushed::Events(events) => events.iter().map(|event| event.seq).collect(),
        Pushed::Transient(_) => Vec::new(),
    }
}

#[tokio::test]
async fn creating_stores_the_snapshot_and_the_first_event() {
    let home = Home::new();
    let handle = home.create(Script::new([])).await;
    let log = home.log(handle.id());
    assert_eq!(kinds(&log), ["session.created"]);
    assert_eq!(log[0].cause, Some(id("cmd-0")));
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话：{:?}", log[0]);
    };
    // 先落 blob，再写引用它的事件：它记的哈希取得出快照。
    let blobs = Blobs::new(home.root.blobs(&alice_account()));
    let bytes = blobs.get(&created.policy).expect("快照在 blob 里");
    let snapshot = Snapshot::from_bytes(&bytes).expect("读得懂");
    assert_eq!(snapshot.persona, "engineer");
    assert_eq!(snapshot.hash(), created.policy);
}

#[tokio::test]
async fn a_turn_is_stored_then_pushed_then_answered() {
    let home = Home::new();
    let script = Script::new([Play::Says("你好。")]);
    let handle = home.create(script.clone()).await;
    let mut pushes = watch(&handle).await;
    let outcome = ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    let Outcome::Accepted { events } = outcome else {
        panic!("应该接受：{outcome:?}");
    };
    // 先见结果，后见回应：回应到的时候，它产生的事件已经在推送里了，不用再等。
    let first = tokio::time::timeout(Duration::ZERO, tokio::task::unconstrained(pushes.next()))
        .await
        .expect("回应到的时候，推送已经在了")
        .expect("订阅没断");
    let first_seqs = seqs(&first);
    assert!(
        events.iter().all(|seq| first_seqs.contains(seq)),
        "{events:?} 应该在 {first_seqs:?} 里"
    );
    let mut pushed = vec![first];
    pushed.extend(until_turn_ends(&mut pushes).await);

    // 磁盘上的，除了造会话那一条，都照先后推过来了。
    let log = home.log(handle.id());
    let all: Vec<Seq> = pushed.iter().flat_map(|pushed| seqs(pushed)).collect();
    let stored: Vec<Seq> = log.iter().skip(1).map(|event| event.seq).collect();
    assert_eq!(all, stored);
    assert_eq!(
        kinds(&log).first().copied(),
        Some("session.created"),
        "{:?}",
        kinds(&log)
    );
    for kind in [
        "message.user",
        "turn.started",
        "model.called",
        "message.assistant",
        "turn.ended",
    ] {
        assert!(
            kinds(&log).contains(&kind),
            "{kind} 应该在 {:?} 里",
            kinds(&log)
        );
    }
    // 增量推过来了，在她的回复落盘之前。
    let delta = pushed
        .iter()
        .position(|pushed| matches!(&**pushed, Pushed::Transient(t) if matches!(t.body, TransientBody::ModelDelta(_))))
        .expect("增量推过来了");
    let reply = pushed
        .iter()
        .position(|pushed| matches!(&**pushed, Pushed::Events(events) if events.iter().any(|event| matches!(event.body, Body::MessageAssistant(_)))))
        .expect("回复推过来了");
    assert!(delta < reply);
    assert_eq!(script.requests().len(), 1);
}

#[tokio::test]
async fn a_retryable_error_waits_and_asks_again() {
    let home = Home::new();
    let script = Script::new([
        Play::Fails {
            class: ErrorClass::RateLimited,
            wait_ms: Some(20),
        },
        Play::Says("好了。"),
    ]);
    let handle = home.create(script.clone()).await;
    let mut pushes = watch(&handle).await;
    let started = Instant::now();
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    let pushed = until_turn_ends(&mut pushes).await;
    // 到点才叫醒：供应商说等 20 毫秒，就等了（时钟到毫秒，留 5 毫秒的零头）。
    assert!(
        started.elapsed() >= Duration::from_millis(15),
        "{:?}",
        started.elapsed()
    );
    assert!(
        pushed.iter().any(|pushed| matches!(&**pushed, Pushed::Transient(t) if matches!(t.body, TransientBody::Status(_)))),
        "推了等着重试的状态"
    );
    let requests = script.requests();
    assert_eq!(requests.len(), 2, "到点以后又请求了一次");
    // 什么都没收到的，原样重发。
    assert_eq!(
        requests[0].1.canonical_bytes(),
        requests[1].1.canonical_bytes()
    );
    let log = home.log(handle.id());
    assert!(
        kinds(&log).contains(&"message.assistant"),
        "{:?}",
        kinds(&log)
    );
}

#[tokio::test]
async fn interrupting_a_held_request_cancels_it() {
    let home = Home::new();
    let script = Script::new([Play::Holds]);
    let handle = home.create(script.clone()).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    // 等增量来了，请求就在读流了。
    until_delta(&mut pushes).await;
    let outcome = ask(
        &handle,
        "cmd-2",
        Command::Interrupt {
            queued: Queued::Return,
        },
    )
    .await
    .expect("会话在跑");
    assert!(matches!(outcome, Outcome::Accepted { .. }), "{outcome:?}");
    until_turn_ends(&mut pushes).await;
    let seen = script.requests()[0].0;
    // 叫停是送给端口的，它停下来要一会儿。
    let waited = tokio::time::timeout(Duration::from_secs(5), async {
        while script.cancelled().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "五秒内端口收到了叫停");
    assert_eq!(script.cancelled(), [seen]);
}

#[tokio::test]
async fn a_stopped_session_loads_and_goes_on() {
    let home = Home::new();
    let before = Script::new([Play::Says("你好。")]);
    let handle = home.create(before.clone()).await;
    let session = handle.id().clone();
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    stop(&handle).await;
    assert_eq!(
        ask(&handle, "cmd-2", say("bye")).await,
        Err(Stopped),
        "停下以后不再收命令"
    );

    let after = Script::new([Play::Says("再见。")]);
    let handle = home.load(&session, after.clone()).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-2", say("bye")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    // 这次的请求接着上一次的往下长：前面一条都没变。
    let (first, second) = (&before.requests()[0].1, &after.requests()[0].1);
    assert_eq!(first.system, second.system);
    assert!(second.messages.len() > first.messages.len());
    assert_eq!(second.messages[..first.messages.len()], first.messages[..]);
    let turns = home
        .log(&session)
        .iter()
        .filter(|event| matches!(event.body, Body::TurnEnded(_)))
        .count();
    assert_eq!(turns, 2);
}

#[tokio::test]
async fn the_same_command_twice_is_answered_twice_and_counts_once() {
    let home = Home::new();
    let script = Script::new([Play::Says("你好。")]);
    let handle = home.create(script.clone()).await;
    let mut pushes = watch(&handle).await;
    let first = ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let stored = home.log(handle.id()).len();
    let again = ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    assert_eq!(again, first, "回应照上一次");
    assert_eq!(home.log(handle.id()).len(), stored, "没有再产生事件");
    assert_eq!(script.requests().len(), 1);
}

#[tokio::test]
async fn a_stop_in_the_middle_of_a_turn_is_stored_and_resumed_after_loading() {
    let home = Home::new();
    let before = Script::new([Play::Holds]);
    let handle = home.create(before.clone()).await;
    let session = handle.id().clone();
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_delta(&mut pushes).await;
    stop(&handle).await;
    // 停下等它的事件落了盘才回：那一轮照重启收拾好了，已经在磁盘上。
    let log = home.log(&session);
    let last = log.last().expect("日志不是空的");
    assert!(
        matches!(&last.body, Body::TurnEnded(ended) if ended.reason == EndReason::Restarted),
        "{:?}",
        kinds(&log)
    );

    // 载入：被重启打断的那一轮接着干，不用人再说一句。
    let after = Script::new([Play::Says("接着说完。")]);
    let _handle = home.load(&session, after.clone()).await;
    let log = until_logged(&home, &session, |log| {
        log.iter()
            .filter(|event| matches!(&event.body, Body::TurnEnded(ended) if ended.reason == EndReason::Completed))
            .count()
            == 1
    })
    .await;
    assert_eq!(after.requests().len(), 1, "{:?}", kinds(&log));
}

#[tokio::test]
async fn a_loaded_session_never_goes_back_in_time() {
    let home = Home::new();
    let handle = home.create(Script::new([])).await;
    let session = handle.id().clone();
    stop(&handle).await;
    // 磁盘上的时刻挪到 2100 年：像是系统时间在两次运行之间往回拨了。
    let later = Timestamp::parse("2100-01-01T00:00:00.000Z").expect("时刻合写法");
    let segment = home
        .root
        .session_dir(&alice_account(), &session)
        .join("000000000001.jsonl");
    let text = std::fs::read_to_string(&segment).expect("读得了第一段");
    let moved: String = text
        .lines()
        .map(|line| {
            let mut event = Event::from_line(line).expect("每一行都是事件");
            event.at = later;
            event.to_line() + "\n"
        })
        .collect();
    std::fs::write(&segment, moved).expect("写得回去");

    let handle = home.load(&session, Script::new([Play::Says("好。")])).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    let log = home.log(&session);
    assert!(log.len() > 1);
    assert!(
        log.iter().all(|event| event.at >= later),
        "载入以后的时刻不比日志里的早：{:?}",
        log.iter().map(|event| event.at).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn a_session_nobody_holds_cancels_its_request() {
    let home = Home::new();
    let script = Script::new([Play::Holds]);
    let handle = home.create(script.clone()).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_delta(&mut pushes).await;
    // 没人拿着这个会话了：actor 退出，路上的请求没人要结果了，叫停。
    drop((handle, pushes));
    let waited = tokio::time::timeout(Duration::from_secs(5), async {
        while script.cancelled().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "五秒内端口收到了叫停");
}

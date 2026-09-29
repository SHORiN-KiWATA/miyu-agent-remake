//! 核心进程的后半段（`docs/construction/3-9-主程序和拉起核心（上）.md` 验收第 2 条）：没有连接、也没有在跑的
//! 回合，空闲够久了就退出；有连接的、有回合在跑的不退；收到停的信号，先让在跑的会话有计划地停下；没设
//! key 的，每次请求都回「没有可用的模型」。在进程里跑，请求模型照剧本回。

mod support;

use std::sync::Arc;
use std::time::Duration;

use miyu_core::{Stopped, models, serve};
use miyu_ipc::{ConnectError, Lock};
use miyu_kernel::event::{Body, CallResult, EndReason, ErrorClass};
use miyu_session::testkit::{Play, Script};
use support::{Head, Home, within};

/// 测试里的空闲时限。
const IDLE: Duration = Duration::from_millis(200);

/// 空闲时限的几倍：这么久还没退，就是没退。
const WAIT: Duration = Duration::from_millis(800);

#[tokio::test]
async fn an_idle_core_stops_and_lets_go_of_everything() {
    let home = Home::new();
    let opened = home.open();
    let core = home.core(Arc::new(Script::new([])), &opened.token);
    let stopped = within(
        "空闲退出",
        serve(opened.listener, core, IDLE, std::future::pending()),
    )
    .await;
    assert_eq!(stopped, Stopped::Idle);
    Lock::acquire(&home.root).expect("锁放开了");
    // Windows 上丢掉还在等连接的管道，句柄要等取消了那次等待才真关上，管道名会多留一小会儿；核心进程随后
    // 就退出了，系统替它全关掉。Unix 上套接字文件当场就删了。
    within("连不上了", async {
        while !matches!(
            miyu_ipc::connect(&home.root).await,
            Err(ConnectError::NotRunning)
        ) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
}

#[tokio::test]
async fn a_head_keeps_it_running_until_it_goes() {
    let home = Home::new();
    let opened = home.open();
    let core = home.core(Arc::new(Script::new([])), &opened.token);
    let running = tokio::spawn(serve(opened.listener, core, IDLE, std::future::pending()));
    let head = Head::connect(&home.root).await;
    tokio::time::sleep(WAIT).await;
    assert!(!running.is_finished(), "有头连着，不退");
    drop(head);
    let stopped = within("头走了以后空闲退出", running)
        .await
        .expect("没 panic");
    assert_eq!(stopped, Stopped::Idle);
}

#[tokio::test]
async fn the_idle_clock_starts_when_the_last_head_goes() {
    let home = Home::new();
    let opened = home.open();
    let core = home.core(Arc::new(Script::new([])), &opened.token);
    let idle = Duration::from_secs(1);
    let running = tokio::spawn(serve(opened.listener, core, idle, std::future::pending()));
    // 起来就空闲着；过了大半个时限才来一个头，连着超过一个时限才走。
    tokio::time::sleep(Duration::from_millis(600)).await;
    let head = Head::connect(&home.root).await;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    drop(head);
    // 从头走的那一刻重新算：半个时限以后还在。
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!running.is_finished(), "空闲的钟从最后一个头走的时候算起");
    let stopped = within("空闲退出", running).await.expect("没 panic");
    assert_eq!(stopped, Stopped::Idle);
}

#[tokio::test]
async fn a_running_turn_keeps_it_running_after_the_head_goes() {
    let home = Home::new();
    let opened = home.open();
    let core = home.core(Arc::new(Script::new([Play::Holds])), &opened.token);
    let running = tokio::spawn(serve(opened.listener, core, IDLE, std::future::pending()));
    let mut head = Head::connect(&home.root).await;
    let session = head.create().await;
    head.say(&session, "在吗").await;
    drop(head);
    tokio::time::sleep(WAIT).await;
    assert!(!running.is_finished(), "这一轮还在跑（模型一直不回），不退");
    let mut head = Head::connect(&home.root).await;
    head.interrupt(&session).await;
    drop(head);
    let stopped = within("打断以后空闲退出", running).await.expect("没 panic");
    assert_eq!(stopped, Stopped::Idle);
}

#[tokio::test]
async fn a_stop_signal_stops_the_running_sessions_first() {
    let home = Home::new();
    let opened = home.open();
    let core = home.core(Arc::new(Script::new([Play::Holds])), &opened.token);
    let (stop, stopping) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(serve(
        opened.listener,
        core,
        Duration::from_secs(600),
        async {
            if stopping.await.is_err() {
                std::future::pending::<()>().await;
            }
        },
    ));
    let mut head = Head::connect(&home.root).await;
    let session = head.create().await;
    head.say(&session, "在吗").await;
    stop.send(()).expect("还在跑");
    let stopped = within("收到信号停下", running).await.expect("没 panic");
    assert_eq!(stopped, Stopped::Signal);
    let ended = home
        .log(&session)
        .into_iter()
        .find_map(|event| match event.body {
            Body::TurnEnded(ended) => Some(ended.reason),
            _ => None,
        });
    assert_eq!(
        ended,
        Some(EndReason::Restarted),
        "跑到一半的回合记成重启了"
    );
}

#[tokio::test]
async fn without_a_key_every_request_says_there_is_no_model() {
    for key in [None, Some("  ".to_string())] {
        let home = Home::new();
        let opened = home.open();
        let env = models::ModelEnv {
            key: key.clone(),
            ..models::ModelEnv::default()
        };
        let models = models::from_env(&env).expect("造得出");
        let core = home.core(models, &opened.token);
        let running = tokio::spawn(serve(opened.listener, core, IDLE, std::future::pending()));
        let mut head = Head::connect(&home.root).await;
        let session = head.create().await;
        head.say(&session, "在吗").await;
        let log = home.until_turn_ends(&session).await;
        let called = log.iter().find_map(|event| match &event.body {
            Body::ModelCalled(called) => Some(called.clone()),
            _ => None,
        });
        let called = called.unwrap_or_else(|| panic!("{key:?}：记了一次请求：{log:?}"));
        assert_eq!(called.result, CallResult::Error, "{key:?}");
        let error = called.error.expect("出错的带原因");
        assert_eq!(error.class, ErrorClass::Auth, "{key:?}");
        assert_eq!(error.message, "no model: set DEEPSEEK_API_KEY");
        assert_eq!(called.endpoint, None, "没发出去");
        drop(head);
        within("空闲退出", running).await.expect("没 panic");
    }
}

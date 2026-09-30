//! 派到一半的空子会话（施工 7-8，`docs/blueprint/agents.md` 第一条第 7 条，`protocol.md`「会话表」第 8 条）：真核心走一遍。
//! 父会话派出去一个子代理，子会话造好了、交代也送进去了，父会话却没来得及记下 `job.started` 就崩了（日志截在派它的那条
//! 回复后面）：再载入父会话时，子会话挪进回收处；记下了的照留。

mod support;

use std::sync::Arc;

use miyu_kernel::event::Body;
use miyu_kernel::id::SessionId;
use miyu_session::testkit::{Play, Script};
use support::deleting::{Router, agent, base_tools, in_place, started_child, trashed};
use support::{Client, Home, alice, until};

/// 主会话派一个子代理、说一句；子代理停在请求上。
fn scripts() -> Router {
    Router(Arc::new(vec![
        (
            "派一个去查",
            Script::new([
                Play::calls(&[("agent", &agent("查 A"))]),
                Play::Says("派出去了。"),
            ]),
        ),
        ("查 A", Script::new([Play::Holds])),
    ]))
}

/// 派出去，等子代理停在请求上，核心有计划地停下：交回主会话、子会话。
async fn dispatched(home: &Home, router: &Router) -> (String, SessionId) {
    let core = home.core_with_models(Arc::new(router.clone()), base_tools());
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let parent = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &parent, "派一个去查").await;
    home.until_turns(&parent, 1).await;
    let child = started_child(&home.log(&parent)).expect("派出去了子代理");
    until("子代理停在请求上", || {
        !router.0[1].1.requests().is_empty()
    })
    .await;
    core.stop_sessions().await;
    (parent, child)
}

/// 把会话 `session` 的日志截到派子代理的那条回复为止：父会话记下 `job.started` 之前崩了的样子。`kept` 是留下几条工具
/// 结果：一次派了两个、只记下了第一个的样子。
fn crash_before_the_job_was_recorded(home: &Home, session: &str, kept: usize) {
    let id = SessionId::parse(session).expect("合写法");
    let dir = home.root.session_dir(&alice(), &id);
    let segment = std::fs::read_dir(&dir)
        .expect("读得了")
        .map(|entry| entry.expect("读得了").path())
        .find(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .expect("有一段日志");
    let text = std::fs::read_to_string(&segment).expect("读得回来");
    let mut results = 0;
    let kept: String = text
        .split_inclusive('\n')
        .take_while(|line| {
            if line.contains(r#""kind":"tool.result""#) {
                results += 1;
            }
            results <= kept
        })
        .collect();
    std::fs::write(&segment, kept).expect("写得进");
}

#[tokio::test]
async fn a_child_its_parent_never_recorded_goes_to_the_trash_when_the_parent_loads() {
    let home = Home::new();
    let router = scripts();
    let (parent, child) = dispatched(&home, &router).await;
    crash_before_the_job_was_recorded(&home, &parent, 0);
    let mut client = Client::connect(home.core_with_models(Arc::new(router), base_tools()));
    client.hello().await;
    // 随便一个要载入它的命令：订阅。
    client.subscribe("s1", &parent).await;
    assert!(!in_place(&home, child.as_str()), "空子会话挪走了");
    assert!(trashed(&home, child.as_str()).join("deleted_at").exists());
    assert!(in_place(&home, &parent), "父会话还在");
    let log = home.log(&parent);
    assert!(
        log.iter()
            .any(|event| matches!(event.body, Body::TurnEnded(_))),
        "父会话照崩了收尾"
    );
    assert!(started_child(&log).is_none(), "父会话从来没记下它");
}

#[tokio::test]
async fn a_child_its_parent_recorded_stays() {
    let home = Home::new();
    let router = scripts();
    let (parent, child) = dispatched(&home, &router).await;
    let mut client = Client::connect(home.core_with_models(Arc::new(router), base_tools()));
    client.hello().await;
    client.subscribe("s1", &parent).await;
    assert!(in_place(&home, child.as_str()), "记下了的子会话照留");
}

/// 一次派了两个，父会话只记下了一个就崩了：认不得的那一个收掉，记下了的照留。
#[tokio::test]
async fn only_the_child_its_parent_never_recorded_goes() {
    let home = Home::new();
    let router = Router(Arc::new(vec![
        (
            "派两个去查",
            Script::new([
                Play::calls(&[("agent", &agent("查 A")), ("agent", &agent("查 B"))]),
                Play::Says("派出去了。"),
            ]),
        ),
        ("查 A", Script::new([Play::Holds])),
        ("查 B", Script::new([Play::Holds])),
    ]));
    let core = home.core_with_models(Arc::new(router.clone()), base_tools());
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let parent = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &parent, "派两个去查").await;
    home.until_turns(&parent, 1).await;
    until("两个子代理都停在请求上", || {
        !router.0[1].1.requests().is_empty() && !router.0[2].1.requests().is_empty()
    })
    .await;
    core.stop_sessions().await;
    drop(client);
    let children: Vec<SessionId> = home
        .log(&parent)
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(&result.effects),
            _ => None,
        })
        .flatten()
        .filter_map(|effect| match effect {
            miyu_kernel::event::Effect::JobStarted(started) => started.session.clone(),
            _ => None,
        })
        .collect();
    assert_eq!(children.len(), 2);
    crash_before_the_job_was_recorded(&home, &parent, 1);
    let recorded = started_child(&home.log(&parent)).expect("记下了一个");
    let mut client = Client::connect(home.core_with_models(Arc::new(router), base_tools()));
    client.hello().await;
    client.subscribe("s1", &parent).await;
    for child in &children {
        let stays = *child == recorded;
        assert_eq!(
            in_place(&home, child.as_str()),
            stays,
            "{child}：记下了的照留"
        );
        assert_eq!(!stays, trashed(&home, child.as_str()).exists());
    }
}

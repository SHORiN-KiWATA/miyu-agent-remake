//! 删除会话连子会话（施工 3-8 三补，`docs/blueprint/agents.md` 第七条第 5、6 条）：派出去的子会话一层层停下、一起挪走，它们
//! 和它自己的后台命令整组杀掉、不记回报：真核心走一遍。删一个子会话本身，父会话还在的，照人停掉它：父会话记 `stopped` 的
//! 回报、被叫醒，报过又被留了言、父会话又在等它的也一样（施工 7-7）；父会话已经不在的不送。
//!
//! 几个会话同时请求模型，谁先到不一定：替身照请求里人这边的那句交代分给各自的剧本（`support/deleting.rs` 的 `Router`）。

mod support;

use std::sync::Arc;

use serde_json::json;

use miyu_kernel::event::{Body, ChildReason, Effect, Event};
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::{By, Session};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_store::log::read_events;
use miyu_tool::testkit::{Act, Fake, Held};
use miyu_tool::{Catalog, Tool};

use support::deleting::*;
use support::*;

#[tokio::test]
async fn children_stop_layer_by_layer_and_go_with_their_parent() {
    let home = Home::new();
    let (mine, childs) = (Held::new(&[]), Held::new(&[]));
    let mut tools = miyu_basesystem::tools(&default_resources()).expect("出厂的工具");
    tools.push(
        Fake::new("start", Access::Read, Act::Background(Arc::clone(&mine))) as Arc<dyn Tool>,
    );
    tools.push(
        Fake::new("begin", Access::Read, Act::Background(Arc::clone(&childs))) as Arc<dyn Tool>,
    );
    let tools = Catalog::new(tools).expect("合写法");
    // 主会话派一个子代理、放一条后台命令，说完就闲着；子代理派一个孙代理、放一条后台命令，停在请求上；孙代理也停着。
    let router = Router(Arc::new(vec![
        (
            "派一个去查",
            Script::new([
                Play::calls(&[("subagent", &agent("查 A")), ("start", "{}")]),
                Play::Says("派出去了。"),
            ]),
        ),
        (
            "查 A",
            Script::new([
                Play::calls(&[("subagent", &agent("查 B")), ("begin", "{}")]),
                Play::Holds,
            ]),
        ),
        ("查 B", Script::new([Play::Holds])),
    ]));
    let mut client = Client::connect(home.core_with_models(Arc::new(router.clone()), tools));
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派一个去查").await;
    home.until_turns(&parent, 1).await;
    let child = started_child(&home.log(&parent)).expect("派出去了子代理");
    until("子代理派出孙代理、放出后台命令", || {
        started_jobs(&home.log(child.as_str())) == 2
    })
    .await;
    let grandchild = started_child(&home.log(child.as_str())).expect("派出去了孙代理");
    let mut ids = started_ids(&home.log(child.as_str()));
    ids.sort();
    // 主会话一步里派子代理、放后台命令，谁先领 j1 也不一定：子代理的编号照主会话日志里记它的那一条读（施工 7-8 在 CI 上撞见）。
    let own = home
        .log(&parent)
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(&result.effects),
            _ => None,
        })
        .flatten()
        .find_map(|effect| match effect {
            Effect::JobStarted(started) if started.session.as_ref() == Some(&child) => {
                Some(started.job.to_string())
            }
            _ => None,
        })
        .expect("主会话记着派它的那一条");
    assert_eq!(
        ids,
        [format!("{own}.1"), format!("{own}.2")],
        "子代理派的孙代理、后台命令带着它自己的编号，一起派的谁先领不一定（施工 7-1 补）"
    );
    until("孙代理停在请求上", || {
        !router.0[2].1.requests().is_empty()
    })
    .await;
    until("子代理停在请求上", || {
        router.0[1].1.requests().len() == 2
    })
    .await;

    let (_, reply) = delete(&mut client, "d1", &parent).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    for session in [&parent, child.as_str(), grandchild.as_str()] {
        assert!(!in_place(&home, session), "{session} 原处没了");
        assert!(
            trashed(&home, session).join("deleted_at").exists(),
            "{session} 各自挪进回收处"
        );
        assert!(
            !reported_jobs(&trashed_log(&home, session)),
            "{session} 不记回报"
        );
    }
    assert_eq!(mine.killed(), 1, "它自己的后台命令整组杀掉");
    assert_eq!(childs.killed(), 1, "子代理的后台命令整组杀掉");
    assert!(listed(&mut client, "l1").await.is_empty());
    let reply = client
        .call(
            "x1",
            "session.send",
            json!({"session": child.as_str(), "text": "还在吗"}),
        )
        .await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
}

/// 主会话派一个子代理、说一句，被回报叫醒再说一句；子代理停在请求上。
fn parent_and_child_scripts() -> Router {
    Router(Arc::new(vec![
        (
            "派一个去查",
            Script::new([
                Play::calls(&[("subagent", &agent("查 A"))]),
                Play::Says("派出去了。"),
                Play::Says("知道它停了。"),
            ]),
        ),
        ("查 A", Script::new([Play::Holds])),
    ]))
}

/// 在核心 `core` 上照 [`parent_and_child_scripts`] 派一个子代理，等它停在请求上：交回连接、主会话、子会话。
async fn parent_and_child(
    home: &Home,
    core: Arc<miyu_endpoint::Core>,
    router: &Router,
) -> (Client, String, SessionId) {
    let mut client = Client::connect(core);
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派一个去查").await;
    home.until_turns(&parent, 1).await;
    let child = started_child(&home.log(&parent)).expect("派出去了子代理");
    until("子代理停在请求上", || {
        !router.0[1].1.requests().is_empty()
    })
    .await;
    (client, parent, child)
}

#[tokio::test]
async fn deleting_a_child_stops_it_like_a_person_and_wakes_its_parent() {
    let home = Home::new();
    let router = parent_and_child_scripts();
    let core = home.core_with_models(Arc::new(router.clone()), base_tools());
    let (mut client, parent, child) = parent_and_child(&home, core, &router).await;
    let (_, reply) = delete(&mut client, "d1", child.as_str()).await;
    assert_eq!(
        reply["result"],
        json!({}),
        "正忙的子会话照人停掉它再删：{reply}"
    );
    let log = home.log(&parent);
    let reports: Vec<(&Event, _)> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::ChildReported(reported) => Some((event, reported)),
            _ => None,
        })
        .collect();
    let [(event, reported)] = reports[..] else {
        panic!("主会话记了一条回报：{log:#?}")
    };
    assert_eq!(reported.reason, ChildReason::Stopped);
    assert_eq!(reported.session, child);
    assert!(!reported.by_model, "人停的，不带 by_model");
    assert_eq!(event.by, By::Session(Session { id: child.clone() }));
    home.until_turns(&parent, 2).await;
    assert!(!in_place(&home, child.as_str()), "子会话挪走了");
    assert!(trashed(&home, child.as_str()).join("deleted_at").exists());
    assert!(in_place(&home, &parent), "主会话还在");
    assert_eq!(listed(&mut client, "l1").await, [parent.as_str()]);
}

#[tokio::test]
async fn a_child_whose_parent_is_gone_is_deleted_without_a_report() {
    let home = Home::new();
    let router = parent_and_child_scripts();
    let first = home.core_with_models(Arc::new(router.clone()), base_tools());
    let (client, parent, child) = parent_and_child(&home, first.clone(), &router).await;
    first.stop_sessions().await;
    drop(client);
    // 删主会话删到一半崩了的样子：主会话已经进了回收处，子会话还在原处。
    let (from, to) = (
        home.root
            .session_dir(&alice(), &SessionId::parse(&parent).expect("合写法")),
        trashed(&home, &parent),
    );
    std::fs::create_dir_all(to.parent().expect("有上一级")).expect("建得了");
    std::fs::rename(&from, &to).expect("挪得走");
    let before = read_events(&to).expect("读得回来");
    let mut client = Client::connect(home.core_with_models(Arc::new(router), base_tools()));
    client.hello().await;
    let (_, reply) = delete(&mut client, "d1", child.as_str()).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    assert!(!in_place(&home, child.as_str()));
    assert_eq!(read_events(&to).expect("读得回来"), before, "不送");
}

/// 子代理报过一次、主会话又给它留了言（施工 7-7），它又在干活、主会话又等它一份回报：删它照样照人停掉它，主会话记 `stopped` 的
/// 回报、被叫醒，不再等它。
#[tokio::test]
async fn deleting_a_messaged_child_settles_what_its_parent_waits_for() {
    let home = Home::new();
    let message = json!({"to": "j1", "message": "再查 B"}).to_string();
    let router = Router(Arc::new(vec![
        (
            "派一个去查",
            Script::new([
                Play::calls(&[("subagent", &agent("查 A"))]),
                Play::Says("派出去了。"),
                Play::calls(&[("message_agent", &message)]),
                Play::Says("留了言。"),
                Play::Says("知道它停了。"),
            ]),
        ),
        ("查 A", Script::new([Play::Says("A 查完了。"), Play::Holds])),
    ]));
    let core = home.core_with_models(Arc::new(router.clone()), base_tools());
    let mut client = Client::connect(core);
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派一个去查").await;
    // 派出去的一轮、回报叫醒她留言的一轮；子代理的第二次请求停在路上。
    home.until_turns(&parent, 2).await;
    until("子代理收到留言、又在干活", || {
        router.0[1].1.requests().len() == 2
    })
    .await;
    let child = started_child(&home.log(&parent)).expect("派出去了子代理");
    let (_, reply) = delete(&mut client, "d1", child.as_str()).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let reasons: Vec<ChildReason> = home
        .log(&parent)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ChildReported(reported) => Some(reported.reason),
            _ => None,
        })
        .collect();
    assert_eq!(reasons, [ChildReason::Done, ChildReason::Stopped]);
    home.until_turns(&parent, 3).await;
    assert!(!in_place(&home, child.as_str()));
}

/// 删一个孙会话（施工 7-1 补）：子会话派的编号带着它自己的 `j1`，孙代理是 `j1.1`；照人停掉孙会话，子会话记的 `stopped` 回报
/// 也是 `j1.1`，照造孙会话的命令编号读回来的。
#[tokio::test]
async fn deleting_a_grandchild_reports_it_under_its_prefixed_job() {
    let home = Home::new();
    let router = Router(Arc::new(vec![
        (
            "派一个去查",
            Script::new([
                Play::calls(&[("subagent", &agent("查 A"))]),
                Play::Says("派出去了。"),
            ]),
        ),
        (
            "查 A",
            Script::new([Play::calls(&[("subagent", &agent("查 B"))]), Play::Holds]),
        ),
        ("查 B", Script::new([Play::Holds])),
    ]));
    let core = home.core_with_models(Arc::new(router.clone()), base_tools());
    let mut client = Client::connect(core);
    client.hello().await;
    let work = home.work.to_string_lossy().into_owned();
    let parent = client.create("c1", &work).await;
    client.say("c2", &parent, "派一个去查").await;
    home.until_turns(&parent, 1).await;
    let child = started_child(&home.log(&parent)).expect("派出去了子代理");
    until("子代理派出孙代理", || {
        started_jobs(&home.log(child.as_str())) == 1
    })
    .await;
    let grandchild = started_child(&home.log(child.as_str())).expect("派出去了孙代理");
    until("孙代理停在请求上", || {
        !router.0[2].1.requests().is_empty()
    })
    .await;

    let (_, reply) = delete(&mut client, "d1", grandchild.as_str()).await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    let reported: Vec<(String, ChildReason)> = home
        .log(child.as_str())
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ChildReported(reported) => Some((reported.job.to_string(), reported.reason)),
            _ => None,
        })
        .collect();
    assert_eq!(reported, [("j1.1".to_string(), ChildReason::Stopped)]);
    assert!(!in_place(&home, grandchild.as_str()), "孙会话挪走了");
    assert!(in_place(&home, child.as_str()), "子会话还在");
}

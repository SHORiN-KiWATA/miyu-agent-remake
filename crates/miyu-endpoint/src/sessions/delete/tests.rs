//! 删子会话在表的锁里停它、给父会话送回报（施工 7-8，`docs/blueprint/protocol.md` 的 `session.delete`）：父会话记下
//! `stopped` 的那一刻，别人拿不到表的锁，也就叫不醒这个子会话；等拿到锁时它已经删掉了。原来这一步在拿锁之前经端口来回，
//! 父会话记下 `stopped` 以后、删的那一头拿到锁之前，有人给子会话发一句，它就又开了一轮，删的时候说有回合在进行、删不了，
//! 父会话却以为它停了。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Effect};
use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_kernel::session::Command;
use miyu_session::Pushed;
use miyu_session::testkit::{Play, Script};
use miyu_store::env::{Env, Platform};
use miyu_store::log::read_events;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

use super::super::{Opening, admin};
use crate::Core;

/// 一句话。
fn say(words: &str) -> Command {
    Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
    }
}

#[tokio::test]
async fn no_one_wakes_the_child_between_its_stopped_report_and_its_deletion() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("miyu-endpoint-delete-{}-{n}", std::process::id()));
    let root = DataRoot::locate(&Env {
        platform: Platform::current(),
        miyu_home: Some(dir.clone().into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    })
    .expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("建得了骨架");
    let resources = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let tools =
        Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的工具")).expect("合写法");
    let agent = serde_json::json!({"description": "查", "prompt": "Read it."}).to_string();
    // 父会话派出去以后，父会话、子会话的请求都停住；子会话被叫醒再开的那一轮也停住。
    let script = Script::new([
        Play::calls(&[("agent", &agent)]),
        Play::Holds,
        Play::Holds,
        Play::Holds,
    ]);
    let admin_account = AccountId::parse("alice").expect("合写法");
    let core = Arc::new(Core::new(
        root.clone(),
        ResourceRoot::at(resources),
        Arc::new(script.clone()),
        tools,
        None,
        admin_account.clone(),
        "token".to_string(),
    ));
    let opening = Opening {
        attended: false,
        oneshot: false,
    };
    let cwd = dir.to_string_lossy().into_owned();
    let created = core
        .sessions
        .create(
            &core,
            CommandId::parse("c1").unwrap(),
            "engineer",
            cwd,
            Vec::new(),
            opening,
        )
        .await
        .expect("造得了");
    let parent = core
        .sessions
        .get(&core, &created.id, None, None)
        .await
        .expect("在跑");
    parent
        .handle
        .command(CommandId::parse("c2").unwrap(), admin(&core), say("派一个"))
        .await
        .expect("会话在跑");
    for _ in 0..1000 {
        if script.requests().len() == 3 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(script.requests().len(), 3, "父会话、子会话都停在请求上");
    let child: SessionId = read_events(&root.session_dir(&admin_account, &created.id))
        .expect("读得回来")
        .iter()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => result.effects.iter().find_map(|effect| match effect {
                Effect::JobStarted(started) => started.session.clone(),
                _ => None,
            }),
            _ => None,
        })
        .expect("派出去了");
    let mut pushes = parent.handle.subscribe().await.expect("订阅得了");

    // 父会话一记下子代理停了，就去叫醒它：拿得到表的锁、它还在表里，就给它发一句。在另起的任务里等：测试本身那个 future
    // 排在另起的任务后面才轮到，赶不上删的那一头。
    let waking = {
        let (core, child) = (Arc::clone(&core), child.clone());
        tokio::spawn(async move {
            loop {
                let pushed = pushes.next().await.expect("父会话在跑");
                let reported = matches!(&*pushed, Pushed::Events(events)
                    if events.iter().any(|event| matches!(event.body, Body::ChildReported(_))));
                if reported {
                    break;
                }
            }
            let open = core.sessions.open.lock().await;
            match open.running.get(&child) {
                Some(running) => running
                    .handle
                    .command(CommandId::parse("w1").unwrap(), admin(&core), say("醒醒"))
                    .await
                    .is_ok(),
                None => false,
            }
        })
    };
    let deleting = {
        let (core, child) = (Arc::clone(&core), child.clone());
        tokio::spawn(async move { core.sessions.delete(&core, &child).await })
    };
    let woke = waking.await.expect("没 panic");
    let deleted = deleting.await.expect("没 panic");
    assert_eq!(
        deleted,
        Ok(()),
        "父会话记下它停了，它就删得掉（叫醒过它：{woke}）"
    );
    assert!(!woke, "父会话记下它停了以后，它已经不在表里");
    assert!(
        !root.session_dir(&admin_account, &child).exists(),
        "子会话挪进了回收处"
    );
    std::fs::remove_dir_all(&dir).expect("删得掉");
}

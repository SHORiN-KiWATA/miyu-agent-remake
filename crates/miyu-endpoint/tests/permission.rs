//! 切权限级别（施工 3-8 再补，`docs/blueprint/protocol.md` 的 `session.set_permission_level`）：协议上切到完全放开、
//! 开只读、两样一起换，各记一条 `session.policy_changed`、推给订阅着的头，回应是 `{}`；和现在一样的什么都不记；两格都
//! 不写、格的值不对是参数不对；没有的会话是找不到，停了的会话是停了。回合进行中收紧成只读，这一步里等着的写入当场拦下：
//! 真核心走一遍。

mod support;

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::Barrier;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event, Level, Permission, PolicyChanged, ToolStatus};
use miyu_kernel::origin::By;
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use support::*;

/// 发一条请求，交回回应之前读到的推送和回应：订阅着的会话，推送可能排在回应前面。
async fn request(
    client: &mut Client,
    id: &str,
    method: &str,
    params: Value,
) -> (Vec<Value>, Value) {
    let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
    client.line(&request.to_string()).await;
    client.until_reply(id).await
}

/// 切权限级别，交回回应之前读到的推送和回应。
async fn switch(client: &mut Client, id: &str, params: Value) -> (Vec<Value>, Value) {
    request(client, id, "session.set_permission_level", params).await
}

/// 推送里的 `session.policy_changed`，照先后。
fn policy_pushes(pushed: &[Value]) -> Vec<&Value> {
    pushed
        .iter()
        .filter(|push| push["params"]["event"]["kind"] == json!("session.policy_changed"))
        .map(|push| &push["params"]["event"])
        .collect()
}

/// 一条只换了权限的 `session.policy_changed` 换成的权限；别的是 `None`。
fn switched_to(event: &Event) -> Option<&Permission> {
    match &event.body {
        Body::PolicyChanged(PolicyChanged {
            policy: None,
            permission: Some(permission),
        }) => Some(permission),
        _ => None,
    }
}

#[tokio::test]
async fn switching_over_the_protocol_records_it_and_pushes_it() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.subscribe("s1", &session).await;
    // 切到完全放开、开只读、两样一起换回来：每一次记一条，推给订阅着的头，推送在回应前面。
    let steps = [
        (
            "c2",
            json!({"level": "full"}),
            json!({"level": "full", "read_only": false}),
        ),
        (
            "c3",
            json!({"read_only": true}),
            json!({"level": "full", "read_only": true}),
        ),
        (
            "c4",
            json!({"level": "workspace", "read_only": false}),
            json!({"level": "workspace", "read_only": false}),
        ),
    ];
    for (id, mut params, permission) in steps {
        params["session"] = json!(session);
        let (pushed, reply) = switch(&mut client, id, params).await;
        assert_eq!(reply["result"], json!({}), "{reply}");
        let pushes = policy_pushes(&pushed);
        assert_eq!(pushes.len(), 1, "{pushed:?}");
        let event = pushes[0];
        assert_eq!(
            event["body"],
            json!({ "permission": permission }),
            "{event}"
        );
        assert_eq!(event["by"], json!({"kind": "person", "account": "alice"}));
        assert_eq!(event["cause"], json!(id));
        assert!(event.get("turn").is_none(), "空闲时切的不带回合：{event}");
    }
    let log = home.log(&session);
    let switched: Vec<&Permission> = log.iter().filter_map(switched_to).collect();
    let permission = |level, read_only| Permission { level, read_only };
    assert_eq!(
        switched,
        [
            &permission(Level::Full, false),
            &permission(Level::Full, true),
            &permission(Level::Workspace, false),
        ]
    );
    assert_eq!(log.len(), 4, "造会话一条，切了三次");
}

#[tokio::test]
async fn switching_to_the_same_permission_records_nothing() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.subscribe("s1", &session).await;
    let same = [
        json!({"session": session, "level": "workspace"}),
        json!({"session": session, "read_only": false}),
        json!({"session": session, "level": "workspace", "read_only": false}),
        json!({"session": session, "level": "workspace", "read_only": null}),
    ];
    for (n, params) in same.into_iter().enumerate() {
        let id = format!("c{}", n + 2);
        let (pushed, reply) = switch(&mut client, &id, params).await;
        assert_eq!(reply["result"], json!({}), "{reply}");
        assert!(pushed.is_empty(), "一样的什么都不推：{pushed:?}");
    }
    assert_eq!(home.log(&session).len(), 1, "一样的什么都不记");
}

/// 两格都不写：参数不对，不找会话，没有的会话也是参数不对。
#[tokio::test]
async fn neither_field_is_bad_params() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let missing = "0192f3a0-1111-7abc-8def-001122334455";
    for (id, params) in [
        ("c2", json!({"session": session})),
        (
            "c3",
            json!({"session": session, "level": null, "read_only": null}),
        ),
        ("c4", json!({"session": missing})),
    ] {
        let (_, reply) = switch(&mut client, id, params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
        assert_eq!(reply["error"]["message"], json!("参数不对。"));
    }
    assert_eq!(home.log(&session).len(), 1, "拒绝的什么都不写");
}

#[tokio::test]
async fn values_that_do_not_fit_are_bad_params() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let wrong = [
        json!({"session": session, "level": "read_only"}),
        json!({"session": session, "level": "Full"}),
        json!({"session": session, "level": 2}),
        json!({"session": session, "read_only": "yes"}),
        json!({"session": "nope", "level": "full"}),
        json!({"level": "full"}),
    ];
    for (n, params) in wrong.into_iter().enumerate() {
        let id = format!("c{}", n + 2);
        let (_, reply) = switch(&mut client, &id, params).await;
        assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    }
    assert_eq!(home.log(&session).len(), 1, "拒绝的什么都不写");
}

#[tokio::test]
async fn a_session_that_does_not_exist_is_not_found() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let missing = "0192f3a0-1111-7abc-8def-001122334455";
    let (_, reply) = switch(
        &mut client,
        "c1",
        json!({"session": missing, "read_only": true}),
    )
    .await;
    assert_eq!(reason(&reply), Some("session_not_found"), "{reply}");
}

/// 会话停了（端口一叫就 panic）：切权限级别照别的命令回 `session_stopped`，不当成切了。
#[tokio::test]
async fn a_stopped_session_refuses_the_switch() {
    let home = Home::new();
    let script = Script::new([Play::Panics]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    until("请求到了端口", || script.requests().len() == 1).await;
    let (_, reply) = switch(
        &mut client,
        "c3",
        json!({"session": session, "read_only": true}),
    )
    .await;
    assert_eq!(reason(&reply), Some("session_stopped"), "{reply}");
}

/// 回合进行中收紧成只读（`kernel/session.md`「切权限级别」第 4 条），真核心走一遍：她一次调了读和写，读的停在半路，写的
/// 排在它后面还没跑。这时开只读，写的当场补 `denied`，推给订阅着的头，都在回应前面；读的放行以后这一步齐了，请求之前
/// 注入只读那一块，写的一次都没跑。
#[tokio::test]
async fn tightening_mid_turn_denies_the_waiting_write_on_the_spot() {
    let home = Home::new();
    let gate = Arc::new(Barrier::new(2));
    let look = Fake::new("look", Access::Read, Act::Meets(Arc::clone(&gate)));
    let write = Fake::new("write", Access::Write, Act::Echo);
    let tools = Catalog::new([
        Arc::clone(&look) as Arc<dyn Tool>,
        Arc::clone(&write) as Arc<dyn Tool>,
    ])
    .expect("合写法");
    let script = Script::new([
        Play::calls(&[("look", "{}"), ("write", "{}")]),
        Play::Says("好。"),
    ]);
    let mut client = Client::connect(home.core_with_tools(&script, tools, TOKEN));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.subscribe("s1", &session).await;
    let params = json!({"session": session, "text": "hi"});
    let (_, reply) = request(&mut client, "c2", "session.send", params).await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    until("读的停在半路", || look.calls().len() == 1).await;
    let (pushed, reply) = switch(
        &mut client,
        "c3",
        json!({"session": session, "read_only": true}),
    )
    .await;
    assert_eq!(reply["result"], json!({}), "{reply}");
    // 推送在回应前面：先记切了，再补写的那一次的结果；读的还停着，没有结果。
    let changed = pushed
        .iter()
        .position(|push| push["params"]["event"]["kind"] == json!("session.policy_changed"))
        .expect("推了切权限");
    let results: Vec<(usize, &Value)> = pushed
        .iter()
        .enumerate()
        .filter(|(_, push)| push["params"]["event"]["kind"] == json!("tool.result"))
        .map(|(k, push)| (k, &push["params"]["event"]))
        .collect();
    assert_eq!(results.len(), 1, "只有写的那一次有了结果：{pushed:?}");
    let (at, denied) = results[0];
    assert!(changed < at, "先记切了，再拦写的");
    assert_eq!(denied["body"]["status"], json!("denied"), "{denied}");
    assert_eq!(denied["by"], json!({"kind": "kernel"}), "{denied}");
    assert_eq!(pushed[changed]["params"]["event"]["cause"], json!("c3"));
    // 放行读的：这一步齐了，只读那一块注入以后再请求，她说完这一轮。
    gate.wait().await;
    home.until_turns(&session, 1).await;
    assert!(write.calls().is_empty(), "写的一次都没跑");
    let log = home.log(&session);
    let changed = log
        .iter()
        .find(|event| switched_to(event).is_some())
        .expect("记了切权限");
    assert_eq!(
        switched_to(changed),
        Some(&Permission {
            level: Level::Workspace,
            read_only: true
        })
    );
    assert!(changed.turn.is_some(), "回合进行中切的带上这个回合");
    let (denied, result) = log
        .iter()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) if result.status == ToolStatus::Denied => {
                Some((event, result))
            }
            _ => None,
        })
        .expect("写的被拦下");
    assert_eq!(denied.by, By::Kernel);
    assert_eq!(denied.turn, changed.turn, "拦在这一轮里");
    assert_eq!(denied.seq.get(), changed.seq.get() + 1, "紧跟着切权限");
    assert!(
        matches!(result.blocks.first(), Some(Block::Text(text)) if text.text.contains("read-only")),
        "{:?}",
        result.blocks
    );
    // 她看到过工作区那一块，切到只读用切换那一份写，带上一级（施工 2-7 补）。
    let injected = log.iter().any(|event| {
        matches!(&event.body, Body::ContextInjected(fact)
            if fact.text.starts_with(r#"<permission level="read_only" previous="workspace">"#))
    });
    assert!(injected, "请求之前注入了只读那一块");
    assert_eq!(script.requests().len(), 2);
}

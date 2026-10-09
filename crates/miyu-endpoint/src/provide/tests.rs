//! `provide` 的几条（施工 O-2 上）：访问类别、给哪种会话照写法查，不认识的、空的拒；提供者表照包记，连接断了只拿掉它自己的。
//! 时限照写法查（施工 O-2 下）；提供者的工具带上是谁要的，超时、叫它停、掐掉发 `tool.cancel`，回应先到的不发（`remote_tests.rs`）。

use serde_json::json;
use tokio::sync::mpsc;

use super::*;

fn tool(access: &str, venues: Value) -> ToolParams {
    serde_json::from_value(json!({
        "name": "send_group", "description": "Send to a group.",
        "input_schema": {"type": "object"}, "access": access, "venues": venues
    }))
    .expect("合写法")
}

fn refused_for(params: ToolParams) -> Value {
    let Err(refusal) = checked(params) else {
        panic!("应该拒");
    };
    assert_eq!(refusal.reason, "bad_tool");
    Value::Object(refusal.data.expect("带 data"))
}

#[test]
fn access_and_venues_are_checked() {
    let Checked { spec, venues, .. } =
        checked(tool("outbound", json!(["local", "group"]))).expect("合写法");
    assert_eq!(spec.access, Access::Outbound);
    assert_eq!(
        venues,
        Venues {
            local: true,
            private: false,
            group: true
        }
    );
    assert_eq!(
        refused_for(tool("telepathy", json!(["local"]))),
        json!({"tool": "send_group", "problem": "access"})
    );
    assert_eq!(
        refused_for(tool("read", json!([]))),
        json!({"tool": "send_group", "problem": "venues"})
    );
    assert_eq!(
        refused_for(tool("read", json!(["moon"]))),
        json!({"tool": "send_group", "problem": "venues"})
    );
}

/// 提供者表照包记，重新登记的换掉旧的：交出来的是后来那个连接（往它那头发）。
#[tokio::test]
async fn the_table_follows_the_package_and_a_new_registration_replaces_the_old() {
    let provided = Provided::default();
    assert!(provided.peer("onebot").is_none());
    let (first_out, mut first) = mpsc::channel(1);
    let (second_out, mut second) = mpsc::channel(1);
    provided.register("onebot", Peer::new(first_out));
    provided.register("onebot", Peer::new(second_out));
    let peer = provided.peer("onebot").expect("有");
    tokio::spawn(async move { peer.call("tool.call", json!({})).await });
    assert!(second.recv().await.is_some(), "发给后来的那个");
    assert!(first.try_recv().is_err(), "旧的收不到");
}

/// 提供者的工具（施工 O-2 上）：没有连接的、等着时连接断了的，都是暂时不可用，说法同执行器替目录里没有的工具写的。
#[tokio::test]
async fn without_a_live_connection_a_provided_tool_is_unavailable() {
    let provided = Arc::new(Provided::default());
    let remote = RemoteTool::new(
        checked(tool("read", json!(["local"]))).expect("合写法"),
        "onebot",
        Arc::clone(&provided),
        texts(),
    );
    let unavailable = |done: miyu_tool::Done| {
        assert!(done.error);
        assert_eq!(
            done.blocks,
            [miyu_kernel::block::Block::Text(miyu_kernel::block::Text {
                text: "The tool \"send_group\" is not available right now.\n".to_string()
            })]
        );
        assert_eq!(
            done.human,
            Some(
                miyu_kernel::event::Said::new("core/tool-results/unavailable")
                    .with("name", "send_group")
            )
        );
    };
    let progress = || miyu_tool::Progress::new(|_| {});
    unavailable(remote.run(miyu_tool::testkit::call("{}"), progress()).await);
    // 有连接，可是发不出去（那一头关了）。
    let (out, lines) = mpsc::channel(1);
    drop(lines);
    provided.register("onebot", Peer::new(out));
    unavailable(remote.run(miyu_tool::testkit::call("{}"), progress()).await);
}

/// 两句的模板，照资源里的写。
fn texts() -> Texts {
    Texts {
        unavailable: "The tool \"{name}\" is not available right now.\n".to_string(),
        timed_out: "The tool \"{name}\" did not answer within {seconds} seconds.\n".to_string(),
    }
}

/// 时限（施工 O-2 下）：不写是一分钟；一秒到十分钟，出了这个范围的拒，`problem` 是 `timeout`。
#[test]
fn the_timeout_is_checked() {
    let timed = |timeout: Value| {
        let mut params = serde_json::to_value(tool("read", json!(["local"]))).unwrap();
        params["timeout_ms"] = timeout;
        serde_json::from_value::<ToolParams>(params).unwrap()
    };
    let unset = checked(tool("read", json!(["local"]))).expect("合写法");
    assert_eq!(unset.timeout, Duration::from_secs(60));
    let fast = checked(timed(json!(1000))).expect("合写法");
    assert_eq!(fast.timeout, Duration::from_secs(1));
    for bad in [json!(999), json!(600_001)] {
        assert_eq!(
            refused_for(timed(bad)),
            json!({"tool": "send_group", "problem": "timeout"})
        );
    }
}

mod remote_tests;

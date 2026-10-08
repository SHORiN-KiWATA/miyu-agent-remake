//! 反向调用（施工 O-2 上）：发出去的一行带 `core-<n>` 编号、方法和参数；对上编号的回应交给等它的，对不上的不理；连接断了，
//! 在等的和以后发的都了结成连接断了。

use serde_json::json;
use tokio::sync::mpsc;

use super::*;
use crate::wire::Response;

#[tokio::test]
async fn a_call_goes_out_as_a_request_and_its_response_comes_back() {
    let (out, mut lines) = mpsc::channel(8);
    let peer = Peer::new(out);
    let asking = {
        let peer = peer.clone();
        tokio::spawn(async move { peer.call("tool.call", json!({"tool": "echo"})).await })
    };
    let sent: Value = serde_json::from_str(&lines.recv().await.expect("发出去了")).unwrap();
    assert_eq!(
        sent,
        json!({"jsonrpc": "2.0", "id": "core-1", "method": "tool.call", "params": {"tool": "echo"}})
    );
    peer.answer(Response {
        id: "core-9".to_string(),
        outcome: Ok(json!("别人的")),
    });
    peer.answer(Response {
        id: "core-1".to_string(),
        outcome: Ok(json!({"blocks": []})),
    });
    assert_eq!(asking.await.unwrap(), Ok(Ok(json!({"blocks": []}))));
    // 下一条编号往后数。
    let second = {
        let peer = peer.clone();
        tokio::spawn(async move { peer.call("tool.call", json!({})).await })
    };
    let sent: Value = serde_json::from_str(&lines.recv().await.expect("发出去了")).unwrap();
    assert_eq!(sent["id"], json!("core-2"));
    peer.answer(Response {
        id: "core-2".to_string(),
        outcome: Err(json!({"code": -1, "message": "boom"})),
    });
    assert_eq!(
        second.await.unwrap(),
        Ok(Err(json!({"code": -1, "message": "boom"})))
    );
}

#[tokio::test]
async fn closing_ends_what_waits_and_what_comes_after() {
    let (out, mut lines) = mpsc::channel(8);
    let peer = Peer::new(out);
    let waiting = {
        let peer = peer.clone();
        tokio::spawn(async move { peer.call("tool.call", json!({})).await })
    };
    lines.recv().await.expect("发出去了");
    peer.close();
    assert_eq!(waiting.await.unwrap(), Err(Gone));
    assert_eq!(peer.call("tool.call", json!({})).await, Err(Gone));
}

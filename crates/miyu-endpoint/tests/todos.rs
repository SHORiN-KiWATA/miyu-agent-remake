//! 待办（施工 D-3，`docs/blueprint/protocol.md` 的 `subscribe`、推送的 `todos.changed`）：真核心、真的 `todowrite`。还没写过的，
//! `subscribe` 的回应不带 `todos`；她写了一份，订阅着的头收到 `todos.changed`；之后再订阅的，回应里带着当前的这一份。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 发一条请求，交回回应之前读到的推送和回应。
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

/// 推送里的 `todos.changed` 的 `todos`，照先后。
fn changed(pushed: &[Value]) -> Vec<Value> {
    pushed
        .iter()
        .filter(|push| push["params"]["event"]["kind"] == json!("todos.changed"))
        .map(|push| push["params"]["event"]["body"]["todos"].clone())
        .collect()
}

#[tokio::test]
async fn heads_get_the_list_from_subscribe_and_from_pushes() {
    let home = Home::new();
    let todos = json!([
        {"content": "读代码", "status": "completed"},
        {"content": "写测试", "status": "in_progress"}
    ]);
    let args = json!({ "todos": todos }).to_string();
    let script = Script::new([Play::calls(&[("todowrite", &args)]), Play::Says("好。")]);
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("读得出"))
        .expect("合写法");
    let core = home.core_with_tools(&script, tools, TOKEN);
    let mut client = Client::connect(core.clone());
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let created = client
        .call("create-1", "session.create", json!({ "cwd": cwd }))
        .await;
    let session = created["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("应该造出会话：{created}"))
        .to_string();
    let (_, reply) = request(
        &mut client,
        "sub-1",
        "subscribe",
        json!({"session": session, "stream": "events"}),
    )
    .await;
    assert!(reply["result"].get("todos").is_none(), "还没写过：{reply}");
    let (mut pushed, _) = request(
        &mut client,
        "send-1",
        "session.send",
        json!({"session": session, "text": "开工"}),
    )
    .await;
    home.until_turns(&session, 1).await;
    // 推送走订阅的转发任务，可能落在不相干的回应后面：一直问到收到为止，最多 100 次。
    for k in 0..100 {
        if !changed(&pushed).is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let (more, _) = request(&mut client, &format!("list-{k}"), "session.list", json!({})).await;
        pushed.extend(more);
    }
    assert_eq!(
        changed(&pushed),
        std::slice::from_ref(&todos),
        "写了一份，推一次"
    );
    // 另一个头这时订阅：回应里就有当前的这一份。
    let mut other = Client::connect(core);
    other.hello().await;
    let (_, reply) = request(
        &mut other,
        "sub-2",
        "subscribe",
        json!({"session": session, "stream": "events"}),
    )
    .await;
    assert_eq!(reply["result"]["todos"], todos, "{reply}");
}

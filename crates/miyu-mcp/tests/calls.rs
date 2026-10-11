//! 调工具：几种内容照原样读回来；工具自己说出错了不算出错；不认识的工具是服务的错；要补信息的不支持；服务反过来问的回
//! 「没有这个方法」再接着答；等不及丢掉的发取消；服务退出了是断了。

use std::time::Duration;

use serde_json::json;

use miyu_mcp::testkit::{Kind, Script};
use miyu_mcp::{Content, Failed};

use crate::support::connected;

#[tokio::test]
async fn content_comes_back_as_it_was_sent() {
    for script in [Script::modern(), Script::legacy(Kind::LegacyRefuses)] {
        let (client, _) = connected(script).await;
        let echoed = client
            .call("echo", json!({"text": "你好"}))
            .await
            .expect("答得上");
        assert_eq!(echoed.content, [Content::Text("你好".to_string())]);
        assert!(!echoed.is_error);
        let picture = client.call("picture", json!({})).await.expect("答得上");
        assert_eq!(
            picture.content,
            [
                Content::Image {
                    data: "aGk=".to_string(),
                    mime: "image/png".to_string(),
                },
                Content::Text("a picture".to_string()),
            ]
        );
        let structured = client.call("structured", json!({})).await.expect("答得上");
        assert_eq!(structured.structured, Some(json!({"answer": 42})));
    }
}

#[tokio::test]
async fn a_tool_error_is_a_result_and_an_unknown_tool_is_a_server_error() {
    let (client, _) = connected(Script::modern()).await;
    let failed = client.call("fail", json!({})).await.expect("答得上");
    assert!(failed.is_error);
    assert_eq!(failed.content, [Content::Text("broke".to_string())]);
    assert!(matches!(
        client.call("nope", json!({})).await,
        Err(Failed::Rpc { code: -32602, .. })
    ));
}

#[tokio::test]
async fn asking_for_more_input_is_not_supported() {
    let (client, _) = connected(Script::modern()).await;
    assert_eq!(
        client.call("input", json!({})).await,
        Err(Failed::InputRequired)
    );
}

#[tokio::test]
async fn a_request_from_the_server_is_refused_and_the_call_goes_on() {
    let (client, fake) = connected(Script::legacy(Kind::LegacyRefuses)).await;
    let called = tokio::time::timeout(Duration::from_secs(10), client.call("ask", json!({})))
        .await
        .expect("十秒内答完")
        .expect("答得上");
    assert_eq!(called.content, [Content::Text("asked".to_string())]);
    let refused = fake
        .received()
        .into_iter()
        .find(|message| message["id"] == "s1")
        .expect("回了它的请求");
    assert_eq!(refused["error"]["code"], -32601);
}

#[tokio::test]
async fn giving_up_on_a_call_cancels_it() {
    let (client, fake) = connected(Script::modern()).await;
    let waited =
        tokio::time::timeout(Duration::from_millis(100), client.call("slow", json!({}))).await;
    assert!(waited.is_err(), "slow 不回");
    let call_id = fake
        .received()
        .iter()
        .find(|message| message["method"] == "tools/call")
        .map(|message| message["id"].clone())
        .expect("发了调用");
    let cancelled = async {
        loop {
            let found = fake.received().into_iter().find(|message| {
                message["method"] == "notifications/cancelled"
                    && message["params"]["requestId"] == call_id
            });
            if found.is_some() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(10), cancelled)
        .await
        .expect("十秒内收到取消");
    let echoed = client
        .call("echo", json!({"text": "还在"}))
        .await
        .expect("连接照常");
    assert_eq!(echoed.content, [Content::Text("还在".to_string())]);
}

#[tokio::test]
async fn a_server_that_exits_closes_the_connection() {
    let (client, _) = connected(Script::modern()).await;
    let died = tokio::time::timeout(Duration::from_secs(10), client.call("die", json!({})))
        .await
        .expect("十秒内认出断了");
    assert_eq!(died, Err(Failed::Closed));
    tokio::time::timeout(Duration::from_secs(10), client.closed())
        .await
        .expect("十秒内认出断了");
    assert_eq!(client.tools().await, Err(Failed::Closed));
}

#[tokio::test]
async fn a_ping_from_the_server_is_answered() {
    let (client, _) = connected(Script::legacy(Kind::LegacyRefuses)).await;
    let called = tokio::time::timeout(Duration::from_secs(10), client.call("pinged", json!({})))
        .await
        .expect("十秒内答完")
        .expect("答得上");
    assert_eq!(called.content, [Content::Text("pong".to_string())]);
}

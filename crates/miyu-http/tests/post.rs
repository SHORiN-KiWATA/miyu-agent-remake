//! 一次 POST（施工 R-5 补：远程的 embedding）：带着头和 JSON 的请求体发出去，2xx 的交回整个响应体；回的不是 2xx 的交回
//! 状态码和响应体；超过上限、超时的说一句；原话里没有地址和头的值。

use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Post, Proxy, fetcher, post};

async fn send(server: &Server, limit: usize) -> Result<Vec<u8>, miyu_http::Failed> {
    let client = fetcher(Proxy::Off).expect("造得出客户端");
    let url = format!("{}/embeddings?key=sk-in-url", server.base_url);
    let headers = vec![("Authorization".to_string(), "Bearer sk-secret".to_string())];
    post(Post {
        client: &client,
        url: &url,
        headers: &headers,
        body: br#"{"model":"m","input":"hi"}"#,
        timeout: Duration::from_secs(2),
        limit,
    })
    .await
}

fn ok(body: &str) -> Reply {
    Reply {
        status: 200,
        headers: vec![("Content-Type".to_string(), "application/json".to_string())],
        body: vec![Piece::Bytes(body.as_bytes().to_vec())],
    }
}

#[tokio::test]
async fn the_body_and_headers_go_out_and_the_answer_comes_back() {
    let server = Server::start(vec![ok(r#"{"data":[]}"#)]).await;
    assert_eq!(send(&server, 1024).await.expect("成了"), br#"{"data":[]}"#);
    let received = server.received();
    assert_eq!(received[0].method, "POST");
    assert!(
        received[0].path.ends_with("/embeddings?key=sk-in-url"),
        "{}",
        received[0].path
    );
    assert_eq!(
        received[0].header("authorization"),
        Some("Bearer sk-secret")
    );
    assert_eq!(received[0].header("content-type"), Some("application/json"));
    assert_eq!(received[0].body, br#"{"model":"m","input":"hi"}"#);
}

#[tokio::test]
async fn failures_say_what_happened_without_the_address_or_the_key() {
    let server = Server::start(vec![
        Reply::error(401, &[], r#"{"error":"bad key"}"#),
        ok(&"x".repeat(2048)),
        Reply::stream(vec![Piece::Stall]),
    ])
    .await;
    let failed = send(&server, 1024).await.expect_err("401");
    assert_eq!(
        (failed.message.as_str(), failed.status),
        ("HTTP 401", Some(401))
    );
    assert_eq!(failed.body, br#"{"error":"bad key"}"#);
    assert_eq!(
        send(&server, 1024).await.expect_err("太大").message,
        "body over 1024 bytes"
    );
    assert_eq!(
        send(&server, 1024).await.expect_err("超时").message,
        "timed out after 2 seconds"
    );
    drop(server);
    let nobody = Server::start(Vec::new()).await;
    let refused = send(&nobody, 1024).await.expect_err("连不上").message;
    assert!(
        !refused.contains("sk-") && !refused.contains("127.0.0.1"),
        "{refused}"
    );
}

//! 一次 GET（施工 8-7）：带头、带 `ETag`、304、出错的几种、大小上限、总时限，原话里没有地址和头的值。

use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Get, Got, Proxy, fetcher, get};

/// 照 `server` 的地址加 `/models` GET 一次。
async fn fetch(server: &Server, etag: Option<&str>, limit: usize) -> Result<Got, String> {
    let client = fetcher(Proxy::Off).expect("造得出客户端");
    let url = format!("{}/models?key=sk-in-url", server.base_url);
    let headers = vec![("Authorization".to_string(), "Bearer sk-secret".to_string())];
    get(Get {
        client: &client,
        url: &url,
        headers: &headers,
        etag,
        timeout: Duration::from_secs(2),
        limit,
    })
    .await
}

fn ok(body: &str, headers: &[(&str, &str)]) -> Reply {
    Reply {
        status: 200,
        headers: headers
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect(),
        body: vec![Piece::Bytes(body.as_bytes().to_vec())],
    }
}

#[tokio::test]
async fn a_body_comes_back_with_its_etag_and_the_request_carries_the_headers() {
    let server = Server::start(vec![ok("{\"data\":[]}", &[("ETag", "\"v2\"")])]).await;
    let got = fetch(&server, Some("\"v1\""), 1024).await;
    assert_eq!(
        got,
        Ok(Got::Body {
            bytes: b"{\"data\":[]}".to_vec(),
            etag: Some("\"v2\"".to_string()),
        })
    );
    let received = server.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].method, "GET");
    assert_eq!(received[0].path, "/v1/models?key=sk-in-url");
    assert_eq!(
        received[0].header("authorization"),
        Some("Bearer sk-secret")
    );
    assert_eq!(received[0].header("if-none-match"), Some("\"v1\""));
}

#[tokio::test]
async fn not_modified_is_its_own_answer_and_no_etag_sends_none() {
    let server = Server::start(vec![Reply::error(304, &[], "")]).await;
    assert_eq!(fetch(&server, None, 1024).await, Ok(Got::NotModified));
    assert_eq!(server.received()[0].header("if-none-match"), None);
}

#[tokio::test]
async fn failures_say_what_happened_without_the_address_or_the_key() {
    let server = Server::start(vec![
        Reply::error(503, &[], "busy"),
        ok(&"x".repeat(2048), &[]),
        Reply::stream(vec![Piece::Stall]),
    ])
    .await;
    assert_eq!(
        fetch(&server, None, 1024).await,
        Err("HTTP 503".to_string())
    );
    assert_eq!(
        fetch(&server, None, 1024).await,
        Err("body over 1024 bytes".to_string())
    );
    let stalled = fetch(&server, None, 1024).await.expect_err("超时");
    assert_eq!(stalled, "timed out after 2 seconds");
    // 连不上：地址里的 key、头的值都不在原话里。
    drop(server);
    let nobody = Server::start(Vec::new()).await;
    let refused = fetch(&nobody, None, 1024).await.expect_err("连不上");
    assert!(!refused.contains("sk-"), "{refused}");
    assert!(!refused.contains("127.0.0.1"), "{refused}");
}

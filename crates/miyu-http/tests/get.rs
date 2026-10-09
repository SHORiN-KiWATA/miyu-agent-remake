//! 一次 GET（施工 8-7）：带头、带 `ETag`、304、出错的几种、大小上限、总时限，原话里没有地址和头的值。边下边交（施工 R-5
//! 中）：一块一块交给收的一方，超过上限停，收的一方出错照它的原话。

use std::io;
use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Get, Got, Proxy, download, fetcher, get, get_full};

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
    // 同一个理由：fetcher() 走的是同一个 build()，也不该自己带 Accept-Encoding（W-7 补）。
    assert_eq!(received[0].header("accept-encoding"), None);
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

#[tokio::test]
async fn the_full_failure_keeps_the_status_the_headers_and_the_body() {
    // 施工 8-11：`provider.test` 列模型失败时照驱动分类，要状态码、头、响应体；原话和 `get` 的一样。
    let server = Server::start(vec![
        Reply::error(
            401,
            &[("Retry-After", "7")],
            "{\"error\":{\"message\":\"bad key\"}}",
        ),
        Reply::error(404, &[], ""),
    ])
    .await;
    let client = fetcher(Proxy::Off).expect("造得出客户端");
    let url = format!("{}/models", server.base_url);
    let ask = || Get {
        client: &client,
        url: &url,
        headers: &[],
        etag: None,
        timeout: Duration::from_secs(2),
        limit: 1024,
    };
    let failed = get_full(ask()).await.expect_err("401");
    assert_eq!(failed.status, Some(401));
    assert_eq!(failed.message, "HTTP 401");
    assert_eq!(failed.body, b"{\"error\":{\"message\":\"bad key\"}}");
    assert!(
        failed
            .headers
            .iter()
            .any(|(name, value)| name == "retry-after" && value == "7"),
        "{:?}",
        failed.headers
    );
    assert_eq!(get(ask()).await, Err("HTTP 404".to_string()));
    drop(server);
    let nobody = Server::start(Vec::new()).await;
    let url = format!("{}/models", nobody.base_url);
    let refused = get_full(Get {
        client: &client,
        url: &url,
        headers: &[],
        etag: None,
        timeout: Duration::from_secs(2),
        limit: 1024,
    })
    .await
    .expect_err("连不上");
    assert_eq!(refused.status, None, "连不上的没有状态码");
    assert!(refused.body.is_empty());
}

/// 照 `server` 的地址边下边交一次，每一块交给 `each`。
async fn pull(
    server: &Server,
    limit: usize,
    each: &mut (dyn FnMut(&[u8]) -> io::Result<()> + Send),
) -> Result<u64, String> {
    let client = fetcher(Proxy::Off).expect("造得出客户端");
    let url = format!("{}/model.onnx", server.base_url);
    let get = Get {
        client: &client,
        url: &url,
        headers: &[],
        etag: None,
        timeout: Duration::from_secs(2),
        limit,
    };
    download(get, each).await
}

#[tokio::test]
async fn a_download_hands_each_piece_on_and_stops_where_it_should() {
    let pieces = |parts: Vec<Piece>| Reply {
        status: 200,
        headers: Vec::new(),
        body: parts,
    };
    let server = Server::start(vec![
        pieces(vec![
            Piece::Bytes(b"abc".to_vec()),
            Piece::Wait(Duration::from_millis(50)),
            Piece::Bytes(b"def".to_vec()),
        ]),
        ok(&"x".repeat(2048), &[]),
        ok("abc", &[]),
        // 说了 6 个字节，给了 2 个就断开：读到一半断了（没写长度的，对方断开就当读完，下载的一方照大小、SHA-256 核对）。
        Reply {
            status: 200,
            headers: vec![("Content-Length".to_string(), "6".to_string())],
            body: vec![Piece::Bytes(b"ab".to_vec()), Piece::Drop],
        },
        Reply::error(404, &[], "gone"),
        pieces(vec![Piece::Stall]),
    ])
    .await;
    let (mut got, mut handed) = (Vec::new(), 0);
    let mut keep = |chunk: &[u8]| {
        handed += 1;
        got.extend_from_slice(chunk);
        Ok(())
    };
    assert_eq!(pull(&server, 1024, &mut keep).await, Ok(6));
    assert_eq!(got, b"abcdef");
    assert!(handed >= 2, "一块一块交，不攒成一整块：交了 {handed} 次");
    assert_eq!(
        pull(&server, 1024, &mut |_| Ok(())).await,
        Err("body over 1024 bytes".to_string())
    );
    let mut full = |_: &[u8]| Err(io::Error::other("disk full"));
    assert_eq!(
        pull(&server, 1024, &mut full).await,
        Err("disk full".to_string())
    );
    let broken = pull(&server, 1024, &mut |_| Ok(()))
        .await
        .expect_err("断了");
    assert!(!broken.contains("127.0.0.1"), "{broken}");
    assert_eq!(
        pull(&server, 1024, &mut |_| Ok(())).await,
        Err("HTTP 404".to_string())
    );
    assert_eq!(
        pull(&server, 1024, &mut |_| Ok(())).await,
        Err("timed out after 2 seconds".to_string())
    );
}

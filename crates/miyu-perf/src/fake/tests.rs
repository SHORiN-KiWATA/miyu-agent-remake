//! 假模型服务：读请求头、找记号、回答的写法，和真起一个收两个请求。

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::*;

#[test]
fn a_head_is_read_once_the_blank_line_is_there() {
    assert_eq!(
        parse_head(b"POST /v1/chat/completions HTTP/1.1\r\nHost: x\r\n"),
        None
    );
    let data =
        b"POST /v1/chat/completions HTTP/1.1\r\nhost: x\r\ncontent-length: 12\r\n\r\n{\"a\":";
    assert_eq!(
        parse_head(data),
        Some(Head {
            method: "POST".to_string(),
            length: 12,
            size: data.len() - 5,
        })
    );
    let get = b"GET /v1/models HTTP/1.1\r\nHost: x\r\n\r\n";
    assert_eq!(
        parse_head(get).map(|head| (head.method, head.length)),
        Some(("GET".to_string(), 0))
    );
}

#[test]
fn a_marker_is_found_anywhere() {
    assert!(mentions(b"perf-7 at the start", b"perf-7"));
    assert!(mentions(b"in the middle perf-7 here", b"perf-7"));
    assert!(mentions(b"at the end perf-7", b"perf-7"));
    assert!(!mentions(b"perf-70 is not perf-8", b"perf-9"));
    assert!(!mentions(b"short", b"a longer marker"));
    assert!(!mentions(b"anything", b""));
}

#[test]
fn the_reply_is_an_openai_chat_stream() {
    let body = stream_body("你好 \"quoted\"", 1000, 800);
    let chunks: Vec<&str> = body
        .split("\n\n")
        .filter(|chunk| !chunk.is_empty())
        .collect();
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[2], "data: [DONE]");
    let said: serde_json::Value =
        serde_json::from_str(chunks[0].strip_prefix("data: ").unwrap()).unwrap();
    assert_eq!(said["choices"][0]["delta"]["content"], "你好 \"quoted\"");
    let done: serde_json::Value =
        serde_json::from_str(chunks[1].strip_prefix("data: ").unwrap()).unwrap();
    assert_eq!(done["choices"][0]["finish_reason"], "stop");
    assert_eq!(done["usage"]["total_tokens"], 1800);
}

#[test]
fn the_filler_reaches_its_size() {
    for bytes in [0, 1, 500, 3500] {
        let text = filler(bytes);
        assert!(text.len() >= bytes, "{bytes}");
        assert!(text.len() < bytes + 400, "{bytes}");
    }
}

/// 发一个请求，读完回应。
async fn send(base: &str, request: String) -> String {
    let address = base.trim_start_matches("http://").trim_end_matches("/v1");
    let mut socket = TcpStream::connect(address).await.unwrap();
    socket.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).await.unwrap();
    response
}

#[tokio::test]
async fn it_answers_and_hands_over_the_marked_request() {
    let mut fake = Fake::start(100).await.unwrap();
    let models = send(
        &fake.base_url,
        "GET /v1/models HTTP/1.1\r\nHost: x\r\n\r\n".to_string(),
    )
    .await;
    assert!(
        models.ends_with(r#"{"object":"list","data":[]}"#),
        "{models}"
    );

    // 请求体比一次读的大，分几次才读完。
    let body = format!("{{\"text\":\"{}perf-1\"}}", "x".repeat(200_000));
    let request = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let reply = send(&fake.base_url, request).await;
    assert!(reply.starts_with("HTTP/1.1 200 OK\r\n"), "{reply}");
    assert!(reply.ends_with("data: [DONE]\n\n"), "{reply}");

    let arrival = fake.request_with("perf-1").await.unwrap();
    assert_eq!(arrival.body, body.as_bytes());
}

/// 回答里收尾那一块的用量：照请求体估的输入、照正文估的输出（施工 V-2 上）。
fn usage_of(stream: &str) -> serde_json::Value {
    stream
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|data| serde_json::from_str::<serde_json::Value>(data).ok())
        .find_map(|chunk| chunk.get("usage").cloned())
        .expect("收尾那一块带用量")
}

#[test]
fn usage_follows_the_size_of_the_request() {
    let small = answer(&[b'x'; 4_000], "reply");
    let large = answer(&[b'x'; 400_000], "reply");
    assert_eq!(usage_of(&small)["prompt_tokens"], 1_000);
    assert_eq!(usage_of(&large)["prompt_tokens"], 100_000);
    assert!(small.contains("reply"), "{small}");
}

#[test]
fn a_summary_request_gets_a_summary() {
    let body =
        br#"{"messages":[{"content":"Write a detailed summary of the conversation above. ..."}]}"#;
    let answered = answer(body, "ordinary reply");
    assert!(answered.contains("<summary>"), "{answered}");
    assert!(!answered.contains("ordinary reply"), "{answered}");
    let ordinary = answer(b"{\"messages\":[]}", "ordinary reply");
    assert!(!ordinary.contains("<summary>"), "{ordinary}");
}

//! `/ws`（施工 W-9，`web-module.md`「怎么走」第九条第 7 到 9 款）：Origin 对上了才接；两头一帧一行照转，一个字节都不改；
//! 二进制 1003、超过 1 MiB 1009、核心断了 1012；连不上核心发 `web.error` 再关；网页软件的代码里不读本机令牌。

mod support;

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{Bytes, Message};
use tokio_tungstenite::{WebSocketStream, client_async};

use miyu_web::serve::address;

use support::*;

type Browser = WebSocketStream<TcpStream>;

/// 照浏览器的样子连 `/ws`：Origin 是 `origin`。
async fn browser(port: u16, origin: Option<&str>) -> Result<Browser, String> {
    let mut request = format!("ws://127.0.0.1:{port}/ws")
        .into_client_request()
        .expect("合写法");
    if let Some(origin) = origin {
        request
            .headers_mut()
            .insert("Origin", origin.parse().expect("合写法"));
    }
    let stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("连得上");
    client_async(request, stream)
        .await
        .map(|(ws, _)| ws)
        .map_err(|error| error.to_string())
}

/// 下一个不是 ping、pong 的消息。
async fn next(ws: &mut Browser) -> Option<Message> {
    loop {
        match within("下一帧", ws.next()).await {
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
            Some(Ok(message)) => return Some(message),
            _ => return None,
        }
    }
}

fn close_code(message: Option<Message>) -> Option<CloseCode> {
    match message {
        Some(Message::Close(Some(frame))) => Some(frame.code),
        _ => None,
    }
}

#[tokio::test]
async fn frames_and_lines_pass_through_untouched() {
    let home = Home::new();
    let mut core = fake_core(&home);
    let (_, port, _serving) = start(&home, 0, 600).await;
    let mut ws = browser(port, Some(&format!("http://127.0.0.1:{port}")))
        .await
        .expect("接了");
    let connection = within("核心接到连接", core.accept()).await.expect("接得到");
    let (read, mut write) = tokio::io::split(connection);
    let mut lines = BufReader::new(read);
    let hello =
        r#"{"id":"h","jsonrpc":"2.0","method":"hello","params":{"code":"9f03","protocol":[1,1]}}"#;
    ws.send(Message::text(hello)).await.expect("发得出");
    let mut line = String::new();
    within("核心读到", lines.read_line(&mut line))
        .await
        .expect("读得到");
    assert_eq!(line, format!("{hello}\n"), "凭据原样到核心，一个字节不改");
    let chinese = r#"{"text":"你好，Miyu  \t空白照留"}"#;
    ws.send(Message::text(chinese)).await.expect("发得出");
    line.clear();
    within("核心读到", lines.read_line(&mut line))
        .await
        .expect("读得到");
    assert_eq!(line, format!("{chinese}\n"));
    let long = format!(r#"{{"x":"{}"}}"#, "字".repeat(200_000));
    write
        .write_all(format!("{long}\n").as_bytes())
        .await
        .expect("写得进");
    match next(&mut ws).await {
        Some(Message::Text(text)) => assert_eq!(text.as_str(), long, "一行去掉换行是一帧"),
        other => panic!("{other:?}"),
    }
    // 核心那头断了：关 1012。
    drop(write);
    drop(lines);
    assert_eq!(close_code(next(&mut ws).await), Some(CloseCode::Restart));
}

#[tokio::test]
async fn the_origin_must_be_this_site() {
    let home = Home::new();
    let _core = fake_core(&home);
    let (_, port, _serving) = start(&home, 0, 600).await;
    for origin in [
        None,
        Some("http://evil.example"),
        Some("https://127.0.0.1"),
        Some("null"),
    ] {
        let refused = browser(port, origin).await.expect_err("不接");
        assert!(refused.contains("403"), "{origin:?}: {refused}");
    }
    for origin in [
        format!("http://localhost:{port}"),
        format!("http://[::1]:{port}"),
    ] {
        assert!(browser(port, Some(&origin)).await.is_ok(), "{origin}");
    }
}

#[tokio::test]
async fn binary_and_oversized_frames_close_the_socket() {
    let home = Home::new();
    let mut core = fake_core(&home);
    let (_, port, _serving) = start(&home, 0, 600).await;
    let origin = format!("http://127.0.0.1:{port}");
    let mut ws = browser(port, Some(&origin)).await.expect("接了");
    let _first = within("核心接到", core.accept()).await;
    ws.send(Message::Binary(Bytes::from_static(b"\x00\x01")))
        .await
        .expect("发得出");
    assert_eq!(
        close_code(next(&mut ws).await),
        Some(CloseCode::Unsupported)
    );
    let mut ws = browser(port, Some(&origin)).await.expect("接了");
    let _second = within("核心接到", core.accept()).await;
    ws.send(Message::text("x".repeat((1 << 20) + 1)))
        .await
        .expect("发得出");
    assert_eq!(close_code(next(&mut ws).await), Some(CloseCode::Size));
}

/// 关的时候不带着没读的数据关：照原始的字节发一帧超过 1 MiB 的，等服务端关了再读，读到关闭帧 1009、再读到头，不是连接被重置。
/// 带着没读的帧正文直接关，系统回 RST，Windows 上关闭帧会被丢（W-9 验收时 CI 上查出来的；Linux 上读的时候报重置）。
#[tokio::test]
async fn an_oversized_frame_is_closed_without_a_reset() {
    let home = Home::new();
    let mut core = fake_core(&home);
    let (_, port, _serving) = start(&home, 0, 600).await;
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("连得上");
    let handshake = format!(
        "GET /ws HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nOrigin: http://127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream
        .write_all(handshake.as_bytes())
        .await
        .expect("写得进");
    let mut head = Vec::new();
    while !head.ends_with(b"\r\n\r\n") {
        let mut byte = [0u8; 1];
        within("握手的回应", stream.read_exact(&mut byte))
            .await
            .expect("读得到");
        head.push(byte[0]);
    }
    assert!(
        head.starts_with(b"HTTP/1.1 101"),
        "{}",
        String::from_utf8_lossy(&head)
    );
    let _connection = within("核心接到", core.accept()).await;
    let size = (1usize << 20) + 1;
    let mask = [1u8, 2, 3, 4];
    let mut frame = vec![0x81, 0x80 | 127];
    frame.extend_from_slice(&(size as u64).to_be_bytes());
    frame.extend_from_slice(&mask);
    frame.extend((0..size).map(|at| b'x' ^ mask[at % 4]));
    stream.write_all(&frame).await.expect("整帧发得出");
    // 先不读：让服务端先关。
    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut got = Vec::new();
    // 服务端发完关闭帧就关写的一半：马上读到头，不用等它读满两秒。
    tokio::time::timeout(Duration::from_millis(1000), stream.read_to_end(&mut got))
        .await
        .expect("服务端关了写的一半")
        .expect("读到头，不是连接被重置");
    assert_eq!(&got[..2], &[0x88, 2], "一个带关闭码的关闭帧：{got:?}");
    assert_eq!(u16::from_be_bytes([got[2], got[3]]), 1009);
}

#[tokio::test]
async fn an_unreachable_core_is_reported_then_closed() {
    let home = Home::new();
    let (_, port, _serving) = start(&home, 0, 600).await;
    let mut ws = browser(port, Some(&format!("http://127.0.0.1:{port}")))
        .await
        .expect("接了");
    match next(&mut ws).await {
        Some(Message::Text(text)) => {
            let notice: serde_json::Value = serde_json::from_str(text.as_str()).expect("是 JSON");
            assert_eq!(notice["method"], "web.error", "{notice}");
            assert!(notice["params"]["message"].is_string());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(close_code(next(&mut ws).await), Some(CloseCode::Normal));
}

#[tokio::test]
async fn an_open_socket_keeps_it_from_idling() {
    let home = Home::new();
    let mut core = fake_core(&home);
    let (_, port, serving) = start(&home, 0, 1).await;
    let mut ws = browser(port, Some(&format!("http://127.0.0.1:{port}")))
        .await
        .expect("接了");
    let connection = within("核心接到连接", core.accept()).await.expect("接得到");
    // 空闲一秒就退；连着一条 WebSocket，过了两倍多还在。
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(!serving.is_finished(), "连着的不算空闲");
    assert!(address(&home.root).is_some());
    // 走了以后才开始算空闲。
    ws.close(None).await.expect("关得了");
    drop(connection);
    let ran = within("空闲退出", serving).await.expect("没崩");
    assert_eq!(ran, Ok(()));
    assert_eq!(address(&home.root), None);
}

/// 网页软件转发浏览器的连接不读本机令牌（「起草时定的」第 4 条）：照源码查，只有 `open.rs` 要一次性码那一下照终端连。
#[test]
fn the_forwarding_code_never_reads_the_local_token() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for file in ["ws.rs", "serve.rs", "pages.rs", "settings.rs", "lib.rs"] {
        let text = std::fs::read_to_string(src.join(file)).expect("读得到");
        for forbidden in [
            "read_token",
            "miyu_ipc::connect(",
            "connect_or_start(",
            "run/token",
            "\"token\"",
        ] {
            assert!(!text.contains(forbidden), "{file} 里有 {forbidden}");
        }
    }
}

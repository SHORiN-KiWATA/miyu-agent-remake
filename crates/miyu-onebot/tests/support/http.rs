//! 照浏览器的样子找 WebUI（施工 O-16，照网页软件的测试搬一份）：手写的 HTTP 请求、连 `/ws` 的 WebSocket 客户端。

use std::time::Duration;

use futures_util::StreamExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::{WebSocketStream, client_async};

use super::within;

/// 一个 HTTP 回应：状态码、头（小写的名字）、正文。
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Answer {
    /// 名字是 `name`（小写）的头。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// 正文当 JSON 读。
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("正文是 JSON")
    }
}

/// 手写一个请求：`method` `path`，Host 是 `host`，另带 `extra` 几行头。
pub async fn request(
    port: u16,
    method: &str,
    path: &str,
    host: &str,
    extra: &[(&str, &str)],
) -> Answer {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .await
        .expect("连得上");
    let mut text = format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    for (name, value) in extra {
        text.push_str(&format!("{name}: {value}\r\n"));
    }
    text.push_str("\r\n");
    stream.write_all(text.as_bytes()).await.expect("写得进");
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), stream.read_to_end(&mut bytes))
        .await
        .expect("十秒内回了")
        .expect("读得到");
    let split = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("有头");
    let head = String::from_utf8_lossy(&bytes[..split]).to_string();
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .expect("有状态码");
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    Answer {
        status,
        headers,
        body: bytes[split + 4..].to_vec(),
    }
}

/// `GET path`，Host 照 `127.0.0.1:<port>` 写对。
pub async fn get(port: u16, path: &str, extra: &[(&str, &str)]) -> Answer {
    request(port, "GET", path, &format!("127.0.0.1:{port}"), extra).await
}

/// 连着 `/ws` 的浏览器。
pub type Browser = WebSocketStream<TcpStream>;

/// 照浏览器的样子连 `/ws`：Origin 是 `origin`。被拒的交回原话（里面有状态码）。
pub async fn browser(port: u16, origin: Option<&str>) -> Result<Browser, String> {
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
pub async fn next(ws: &mut Browser) -> Option<Message> {
    loop {
        match within("下一帧", ws.next()).await {
            Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
            Some(Ok(message)) => return Some(message),
            _ => return None,
        }
    }
}

/// 关闭帧里的关闭码。
pub fn close_code(message: Option<Message>) -> Option<CloseCode> {
    match message {
        Some(Message::Close(Some(frame))) => Some(frame.code),
        _ => None,
    }
}

//! 测试用的假 DeepSeek：本机回环上几十行的 HTTP/1.1。第几个连接回剧本里的第几份，响应体一片一片地
//! 写，写完关连接；可以停住不动，等对方断开。收到的请求体都记下来。

#![allow(dead_code, reason = "两个测试各用其中一部分")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use miyu_http::{Endpoint, Proxy};
use miyu_kernel::id::{ModelName, ProviderId};
use miyu_try::Bench;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// 响应体的一步。
#[derive(Debug, Clone)]
pub enum Piece {
    /// 写这些字节。
    Bytes(Vec<u8>),
    /// 停住不动，直到对方断开。
    Stall,
}

/// 一个在听的假 DeepSeek。
pub struct Fake {
    /// 它的地址，`http://127.0.0.1:端口`。
    pub url: String,
    received: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl Fake {
    /// 开始听：第几个连接回 `replies` 里的第几份，回完了就不再接。
    pub async fn start(replies: Vec<Vec<Piece>>) -> Fake {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("本机回环上开得了端口");
        let url = format!(
            "http://{}",
            listener.local_addr().expect("听着的端口有地址")
        );
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);
        tokio::spawn(async move {
            for reply in replies {
                let (mut socket, _) = listener.accept().await.expect("试玩台连得上来");
                let body = read_request(&mut socket).await;
                log.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(body);
                answer(&mut socket, reply).await;
            }
        });
        Fake { url, received }
    }

    /// 收到的请求体，照 JSON 读。
    pub fn bodies(&self) -> Vec<Value> {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|body| serde_json::from_slice(body).expect("试玩台发的请求体是 JSON"))
            .collect()
    }
}

/// 读一个请求：头读到空行，再照 `Content-Length` 读请求体。
async fn read_request(socket: &mut TcpStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    let head_end = loop {
        let n = socket.read(&mut buffer).await.expect("读得了请求");
        assert!(n > 0, "请求没读全就断了");
        bytes.extend_from_slice(&buffer[..n]);
        if let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let head = String::from_utf8_lossy(&bytes[..head_end]).to_ascii_lowercase();
    let length: usize = head
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .map(|value| value.trim().parse().expect("Content-Length 是个数"))
        .unwrap_or(0);
    while bytes.len() < head_end + length {
        let n = socket.read(&mut buffer).await.expect("读得了请求体");
        assert!(n > 0, "请求体没读全就断了");
        bytes.extend_from_slice(&buffer[..n]);
    }
    bytes[head_end..head_end + length].to_vec()
}

/// 回一份：200、事件流、写完关连接。
async fn answer(socket: &mut TcpStream, reply: Vec<Piece>) {
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
    socket
        .write_all(head.as_bytes())
        .await
        .expect("写得了响应头");
    for piece in reply {
        match piece {
            Piece::Bytes(bytes) => {
                socket.write_all(&bytes).await.expect("写得了响应体");
                socket.flush().await.expect("写得出去");
            }
            Piece::Stall => {
                let mut buffer = [0u8; 64];
                while matches!(socket.read(&mut buffer).await, Ok(n) if n > 0) {}
                return;
            }
        }
    }
}

/// 事件流里的一段：`data: {…}` 加空行。
fn chunk(value: &Value) -> Piece {
    Piece::Bytes(format!("data: {value}\n\n").into_bytes())
}

/// 一段增量。
fn delta(delta: Value) -> Piece {
    chunk(&json!({"choices": [{"index": 0, "delta": delta, "finish_reason": null}]}))
}

/// 开头那一段：角色。
pub fn opening() -> Piece {
    delta(json!({"role": "assistant", "content": ""}))
}

/// 一段思考。
pub fn thinking(text: &str) -> Piece {
    delta(json!({"reasoning_content": text}))
}

/// 一段回复。
pub fn text(text: &str) -> Piece {
    delta(json!({"content": text}))
}

/// 说完了：`stop`，带着用量（输入多少、命中多少、输出多少），再是 `[DONE]`。
pub fn finish(prompt: u64, hit: u64, output: u64) -> Vec<Piece> {
    vec![
        chunk(&json!({
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
            "usage": {
                "prompt_tokens": prompt,
                "completion_tokens": output,
                "total_tokens": prompt + output,
                "prompt_tokens_details": {"cached_tokens": hit},
                "prompt_cache_hit_tokens": hit,
                "prompt_cache_miss_tokens": prompt - hit
            }
        })),
        Piece::Bytes(b"data: [DONE]\n\n".to_vec()),
    ]
}

/// 一个临时的数据根，测试完删掉。
pub fn scratch(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("现在在 1970 年以后")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "miyu-try-test-{name}-{}-{stamp}",
        std::process::id()
    ))
}

/// 连假 DeepSeek 的试玩台：数据写进 `root`，不走代理。
pub fn bench(url: &str, root: &Path) -> Bench {
    Bench {
        endpoint: Endpoint::new(url, "test-key"),
        provider: ProviderId::parse("deepseek").expect("供应商编号合写法"),
        model: ModelName::parse("deepseek-flash").expect("模型名合写法"),
        system: miyu_try::SYSTEM.to_string(),
        root: root.to_path_buf(),
        proxy: Proxy::Off,
        idle: Duration::from_secs(10),
    }
}

/// 会话日志：数据根底下唯一的那个会话目录里的每一行，照 JSON 读，照先后。
pub fn log_lines(dir: &Path) -> Vec<Value> {
    let mut segments: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("会话目录读得了")
        .map(|entry| entry.expect("会话目录里的一项读得了").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "jsonl")
        })
        .collect();
    segments.sort();
    segments
        .iter()
        .flat_map(|path| {
            std::fs::read_to_string(path)
                .expect("日志的一段读得了")
                .lines()
                .map(|line| serde_json::from_str(line).expect("日志的每一行是 JSON"))
                .collect::<Vec<Value>>()
        })
        .collect()
}

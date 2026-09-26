//! 测试用的假服务器：本机回环上几十行的 HTTP/1.1，照剧本回。第几个连接回剧本里的第几份；响应体用
//! 关连接来结束，一片一片地写，中间可以等一会儿、停住不动、直接断开。收到的请求都记下来。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

/// 一份回应。
#[derive(Debug, Clone)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<Piece>,
}

/// 响应体的一步。
#[derive(Debug, Clone)]
pub enum Piece {
    /// 写这些字节。
    Bytes(Vec<u8>),
    /// 等一会儿。
    Wait(Duration),
    /// 停住不动，直到对方断开。
    Stall,
    /// 直接断开。
    Drop,
}

impl Reply {
    /// 200，事件流。
    pub fn stream(pieces: Vec<Piece>) -> Reply {
        Reply {
            status: 200,
            headers: vec![("Content-Type".to_string(), "text/event-stream".to_string())],
            body: pieces,
        }
    }

    /// 一个出错的状态，带着头和响应体。
    pub fn error(status: u16, headers: &[(&str, &str)], body: &str) -> Reply {
        Reply {
            status,
            headers: headers
                .iter()
                .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                .collect(),
            body: vec![Piece::Bytes(body.as_bytes().to_vec())],
        }
    }
}

/// 收到的一个请求。
#[derive(Debug, Clone)]
pub struct Received {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Received {
    /// 名字不分大小写地找一个头。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// 跑着的假服务器。
pub struct Server {
    /// `http://127.0.0.1:<端口>/v1`：驱动的路径接在后面。
    pub base_url: String,
    received: Arc<Mutex<Vec<Received>>>,
    /// 对方断开了几个连接。
    closed: watch::Receiver<usize>,
}

impl Server {
    /// 在本机回环上起一个，端口由系统挑。
    pub async fn start(replies: Vec<Reply>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("绑得上本机回环");
        let port = listener.local_addr().expect("有地址").port();
        let received = Arc::new(Mutex::new(Vec::new()));
        let (closed_tx, closed) = watch::channel(0);
        let closed_tx = Arc::new(closed_tx);
        let log = Arc::clone(&received);
        tokio::spawn(async move {
            for reply in replies {
                let Ok((socket, _)) = listener.accept().await else {
                    return;
                };
                let log = Arc::clone(&log);
                let closed_tx = Arc::clone(&closed_tx);
                tokio::spawn(async move {
                    serve(socket, reply, &log).await;
                    closed_tx.send_modify(|count| *count += 1);
                });
            }
        });
        Server {
            base_url: format!("http://127.0.0.1:{port}/v1"),
            received,
            closed,
        }
    }

    /// 收到的请求，照先后。
    pub fn received(&self) -> Vec<Received> {
        self.received
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 等到有 `count` 个连接断开，最多等五秒。
    pub async fn wait_closed(&mut self, count: usize) {
        let waited = tokio::time::timeout(
            Duration::from_secs(5),
            self.closed.wait_for(|closed| *closed >= count),
        )
        .await;
        assert!(waited.is_ok(), "五秒内对方没有断开连接");
    }
}

/// 一个连接：读请求，照剧本回。
async fn serve(mut socket: TcpStream, reply: Reply, log: &Mutex<Vec<Received>>) {
    let Some(request) = read_request(&mut socket).await else {
        return;
    };
    log.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(request);
    let mut head = format!("HTTP/1.1 {} X\r\n", reply.status);
    for (name, value) in &reply.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("Connection: close\r\n\r\n");
    if socket.write_all(head.as_bytes()).await.is_err() {
        return;
    }
    for piece in reply.body {
        match piece {
            Piece::Bytes(bytes) => {
                if socket.write_all(&bytes).await.is_err() || socket.flush().await.is_err() {
                    return;
                }
            }
            Piece::Wait(duration) => tokio::time::sleep(duration).await,
            Piece::Stall => {
                // 停住，直到对方断开：读到 0 个字节就是断开了。
                let mut buffer = [0u8; 64];
                while let Ok(read) = socket.read(&mut buffer).await {
                    if read == 0 {
                        return;
                    }
                }
                return;
            }
            Piece::Drop => return,
        }
    }
    let _unused = socket.shutdown().await;
}

/// 读一个请求：请求行、头、照 `Content-Length` 读请求体。
async fn read_request(socket: &mut TcpStream) -> Option<Received> {
    let mut data = Vec::new();
    let mut buffer = [0u8; 4096];
    let head_end = loop {
        if let Some(position) = find(&data, b"\r\n\r\n") {
            break position;
        }
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        data.extend_from_slice(&buffer[..read]);
    };
    let head = String::from_utf8_lossy(&data[..head_end]).into_owned();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_string();
    let path = first.next()?.to_string();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .collect();
    let length: usize = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(0);
    let mut body = data[head_end + 4..].to_vec();
    while body.len() < length {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        body.extend_from_slice(&buffer[..read]);
    }
    Some(Received {
        method,
        path,
        headers,
        body,
    })
}

fn find(data: &[u8], needle: &[u8]) -> Option<usize> {
    data.windows(needle.len())
        .position(|window| window == needle)
}

//! 假模型服务：本机回环上的 HTTP/1.1，照 openai-chat 的写法回（施工 V-1）。
//!
//! 每个 `POST` 都回同一段字（一块正文、一块收尾带用量、`[DONE]`），别的请求回一个空的模型列表。用量照请求体、正文的字节数
//! 估，四个字节一个 token；摘要请求（带出厂摘要指令的那一句）回一段摘要，好让长会话照真用时那样到线就压（施工 V-2 上）。它记下每个请求的头
//! 什么时候读全了：量尺照这个时刻算「说一句到模型收到请求」。驱动先把整份请求体编好才发头，所以头到的时刻已经包含了
//! 组装和编码；请求体在回环上传多久不算进去。
//!
//! 和 `miyu_http::testkit` 的假服务器不同：那个照剧本回、回完就不收了；这里一直收，回的字多长由量尺定。

use std::sync::Arc;
use std::time::Instant;

use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::rpc::PATIENCE;

/// 收到的一个请求：头读全的时刻，和请求体。
pub struct Arrival {
    /// 请求头读全的时刻。
    pub at: Instant,
    /// 请求体。
    pub body: Vec<u8>,
}

/// 跑着的假服务。
pub struct Fake {
    /// 给核心写进配置的地址：`http://127.0.0.1:<端口>/v1`。
    pub base_url: String,
    arrivals: mpsc::UnboundedReceiver<Arrival>,
}

impl Fake {
    /// 在本机回环上随便挑个端口起来。每个回答的正文是 `reply_bytes` 个字节上下的字。
    ///
    /// # Errors
    ///
    /// 绑不上端口。
    pub async fn start(reply_bytes: usize) -> Result<Fake, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| format!("假模型服务绑不上端口：{e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("假模型服务的端口读不出：{e}"))?
            .port();
        let reply: Arc<str> = filler(reply_bytes).into();
        let (sender, arrivals) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(serve(socket, Arc::clone(&reply), sender.clone()));
            }
        });
        Ok(Fake {
            base_url: format!("http://127.0.0.1:{port}/v1"),
            arrivals,
        })
    }

    /// 等第一个提到 `marker` 的请求，交回它；之前到的别的请求（起标题这些辅助请求）跳过。
    ///
    /// # Errors
    ///
    /// 服务停了、等太久。
    pub async fn request_with(&mut self, marker: &str) -> Result<Arrival, String> {
        let waiting = async {
            while let Some(arrival) = self.arrivals.recv().await {
                if mentions(&arrival.body, marker.as_bytes()) {
                    return Ok(arrival);
                }
            }
            Err("假模型服务停了".to_string())
        };
        tokio::time::timeout(PATIENCE, waiting).await.map_err(|_| {
            format!(
                "等了 {} 秒，模型没收到带「{marker}」的请求",
                PATIENCE.as_secs()
            )
        })?
    }
}

/// 一个连接：读一个请求，记下，回完关上。
async fn serve(mut socket: TcpStream, reply: Arc<str>, arrivals: mpsc::UnboundedSender<Arrival>) {
    let Some((head, body, at)) = read_request(&mut socket).await else {
        return;
    };
    let response = if head.method == "POST" {
        let mut response =
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
                .to_vec();
        response.extend_from_slice(answer(&body, &reply).as_bytes());
        response
    } else {
        let list = r#"{"object":"list","data":[]}"#;
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{list}",
            list.len()
        )
        .into_bytes()
    };
    // 先记下再回：量尺等到这一条时，核心那边正在收回答，不会因为记晚了少算。
    if arrivals.send(Arrival { at, body }).is_err() {
        return;
    }
    if socket.write_all(&response).await.is_ok() {
        #[expect(clippy::let_underscore_must_use, reason = "回完了，关不干净也不影响量")]
        let _ = socket.shutdown().await;
    }
}

/// 读一个请求：头读全记下时刻，再照 `Content-Length` 读完请求体。连接断了、头读不懂的是 `None`。
async fn read_request(socket: &mut TcpStream) -> Option<(Head, Vec<u8>, Instant)> {
    let mut data = Vec::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let (head, at) = loop {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        data.extend_from_slice(&buffer[..read]);
        if let Some(head) = parse_head(&data) {
            break (head, Instant::now());
        }
    };
    let mut body = data.split_off(head.size);
    while body.len() < head.length {
        let read = socket.read(&mut buffer).await.ok()?;
        if read == 0 {
            return None;
        }
        body.extend_from_slice(&buffer[..read]);
    }
    Some((head, body, at))
}

/// 一个请求的头：方法、请求体多长、头本身占几个字节（连同空行）。
#[derive(Debug, PartialEq, Eq)]
pub struct Head {
    /// `GET`、`POST`……
    pub method: String,
    /// `Content-Length`，没写的是 0。
    pub length: usize,
    /// 头占几个字节，连同结尾的空行：请求体从这里开始。
    pub size: usize,
}

/// 读出请求头；还没读到空行的是 `None`。
pub fn parse_head(data: &[u8]) -> Option<Head> {
    let end = data.windows(4).position(|window| window == b"\r\n\r\n")?;
    let text = String::from_utf8_lossy(&data[..end]);
    let mut lines = text.split("\r\n");
    let method = lines.next()?.split(' ').next()?.to_string();
    let length = lines
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(0);
    Some(Head {
        method,
        length,
        size: end + 4,
    })
}

/// 请求体里有没有 `marker`。从后往前找：人说的最后一句排在请求后面，长会话的请求体有几兆，从前找要白扫一遍。
pub fn mentions(body: &[u8], marker: &[u8]) -> bool {
    !marker.is_empty()
        && body
            .windows(marker.len())
            .rev()
            .any(|window| window == marker)
}

/// 摘要请求认的那一句：出厂摘要指令（`resources/core/compaction/summarize-task.txt`）第二段的开头。
const SUMMARY_MARKER: &[u8] = b"Write a detailed summary of the conversation above";

/// 量尺的摘要：草稿一句、摘要一段，几百字节。
const SUMMARY: &str = "<analysis>量尺的摘要草稿。</analysis>\n<summary>量尺的摘要：用户一直在让模型回长段的字，好让会话长大；没有要接着做的事，也没有改过的文件。The measuring session only grows; nothing is pending.</summary>";

/// 照请求体 `body` 回的事件流（施工 V-2 上）：摘要请求回 [`SUMMARY`]，别的回 `text`；用量照请求体、正文的字节数估。
pub fn answer(body: &[u8], text: &str) -> String {
    let said = match mentions(body, SUMMARY_MARKER) {
        true => SUMMARY,
        false => text,
    };
    stream_body(said, tokens(body.len()), tokens(said.len()))
}

/// 字节数折成 token：四个字节一个，往上取整。
fn tokens(bytes: usize) -> usize {
    bytes.div_ceil(4)
}

/// 回答的事件流：正文一块，收尾一块带用量（输入 `prompt`、输出 `completion` 个 token），最后 `[DONE]`。
pub fn stream_body(text: &str, prompt: usize, completion: usize) -> String {
    let said = json!({"choices": [{"index": 0, "delta": {"role": "assistant", "content": text},
        "finish_reason": null}]});
    let done = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": prompt, "completion_tokens": completion,
            "total_tokens": prompt + completion}});
    format!("data: {said}\n\ndata: {done}\n\ndata: [DONE]\n\n")
}

/// 回答的正文：一段中英混排的字反复，凑到 `bytes` 个字节上下（不切开一个字）。
pub fn filler(bytes: usize) -> String {
    const PIECE: &str = "量尺的回答：这一段字只是占地方，让会话照真用时的样子长大。The measuring reply only takes up room so the session grows the way a real one does. ";
    let mut text = String::with_capacity(bytes + PIECE.len());
    while text.len() < bytes {
        text.push_str(PIECE);
    }
    text
}

#[cfg(test)]
mod tests;

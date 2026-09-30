//! 在一条连接上说 JSON-RPC 2.0，一行一条（`docs/designs/04-核心协议.md` P1）。
//!
//! 照 `crates/miyu-cli/src/rpc.rs` 的做法：读另起一个任务，等回应时来的推送留着，之后照先后交出去。
//! 那边的是 crate 私有的，演示程序用不上，只好照着写一份最小的。

use std::collections::VecDeque;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::sync::mpsc;

use miyu_ipc::Connection;

/// 连着核心的一头。
pub struct Rpc {
    writer: WriteHalf<Connection>,
    incoming: mpsc::UnboundedReceiver<Value>,
    /// 等回应时来的推送。
    held: VecDeque<Value>,
    /// 编号前缀：同一个会话里，两次启动发的命令不撞号（核心按编号去重，`02-内核.md` 不变量 9）。
    prefix: String,
    next: u64,
}

impl Rpc {
    /// 在 `connection` 上说话：另起一个读的任务。要在 tokio 运行时里调。
    pub fn new(connection: Connection) -> Self {
        let (reader, writer) = tokio::io::split(connection);
        let (sender, incoming) = mpsc::unbounded_channel();
        tokio::spawn(read_all(BufReader::new(reader), sender));
        Self {
            writer,
            incoming,
            held: VecDeque::new(),
            prefix: prefix().to_string(),
            next: 0,
        }
    }

    /// 发一条请求，交回它的编号。
    ///
    /// # Errors
    ///
    /// 写不出去。
    pub async fn send(&mut self, method: &str, params: Value) -> io::Result<String> {
        self.next += 1;
        let id = format!("{}-{}", self.prefix, self.next);
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.writer
            .write_all(format!("{request}\n").as_bytes())
            .await?;
        Ok(id)
    }

    /// 发一条请求，等到它的回应，交回 `result`。
    ///
    /// # Errors
    ///
    /// 写不出去、核心断开、被拒绝：交回说清楚的一句。
    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value, Failure> {
        let id = self.send(method, params).await.map_err(Failure::Io)?;
        while let Some(message) = self.incoming.recv().await {
            if message["id"] == json!(id) {
                return match message.get("error") {
                    None => Ok(message["result"].clone()),
                    Some(error) => Err(Failure::Refused(
                        error["message"].as_str().unwrap_or_default().to_string(),
                    )),
                };
            }
            self.held.push_back(message);
        }
        Err(Failure::Disconnected)
    }

    /// 下一条回应或推送，先交留着的。核心断开了是 `None`。
    pub async fn next(&mut self) -> Option<Value> {
        match self.held.pop_front() {
            Some(message) => Some(message),
            None => self.incoming.recv().await,
        }
    }
}

/// 一条请求没成的几种情形。
#[derive(Debug)]
pub enum Failure {
    /// 写不出去。
    Io(io::Error),
    /// 核心断开了。
    Disconnected,
    /// 核心拒绝了，带着它照握手时的语言说的原因。
    Refused(String),
}

/// 读的一头：一行一条，读不懂的不要。读完了（核心断开）就停。
async fn read_all(
    mut reader: BufReader<ReadHalf<Connection>>,
    sender: mpsc::UnboundedSender<Value>,
) {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        if let Ok(message) = serde_json::from_str::<Value>(&line)
            && sender.send(message).is_err()
        {
            return;
        }
    }
}

/// 这个界面发的请求编号的开头：整个进程只定一次，重连以后也一样，核心推来的 `cause` 照它认是不是自己发的（蓝图
/// 「别处来的话」第 1 条）。进程号加启动时刻，两个界面不会撞上。
pub fn prefix() -> &'static str {
    static PREFIX: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    PREFIX.get_or_init(|| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        format!("tui-{:x}{nanos:x}", std::process::id())
    })
}

/// 这个编号是这个界面发的请求。
pub fn owns(id: &str) -> bool {
    id.strip_prefix(prefix())
        .is_some_and(|rest| rest.starts_with('-'))
}

#[cfg(test)]
mod tests {
    use super::{owns, prefix};

    #[test]
    fn our_request_ids_are_known_by_their_prefix() {
        // 核心推来的 `cause` 照它认是不是这个界面发的（蓝图「别处来的话」第 1 条），重连以后也一样。
        assert!(owns(&format!("{}-12", prefix())));
        assert!(!owns("web-12"));
        assert!(
            !owns(&format!("{}9-1", prefix())),
            "开头一样、后面多了字的不算"
        );
    }
}

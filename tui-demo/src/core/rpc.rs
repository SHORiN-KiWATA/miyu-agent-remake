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
        }
    }

    /// 发一条请求，交回它的编号。
    ///
    /// # Errors
    ///
    /// 写不出去。
    pub async fn send(&mut self, method: &str, params: Value) -> io::Result<String> {
        let id = next_id();
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
        let message = self.reply(method, params).await?;
        match message.get("error") {
            None => Ok(message["result"].clone()),
            Some(error) => Err(Failure::Refused(
                error["message"].as_str().unwrap_or_default().to_string(),
            )),
        }
    }

    /// 发一条请求，等它的回应，整条交回（拒了的照样交回，原因码在 `error.data.reason`）；中间来的别的留着。
    ///
    /// # Errors
    ///
    /// 写不出去、核心断开。
    pub async fn reply(&mut self, method: &str, params: Value) -> Result<Value, Failure> {
        let id = self.send(method, params).await.map_err(Failure::Io)?;
        while let Some(message) = self.incoming.recv().await {
            if message["id"] == json!(id) {
                return Ok(message);
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

/// 下一条请求的编号：前缀加整个进程一串往下数的序号，重连以后接着数。核心按编号去重（`02-内核.md` 不变量 9），
/// 两次启动靠前缀分开，同一次启动里的两条连接靠序号分开（2026-10-02：原来每条连接从 1 数起，核心重启以后的撤销、
/// 恢复撞上以前的编号，拿回旧命令的回应，什么都没做）。
pub fn next_id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    format!("{}-{n}", prefix())
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
    fn request_ids_keep_counting_across_reconnects() {
        // 2026-10-02 项目主人报：核心重启以后 /undo 写了「已撤销」却没撤、/restore 也没恢复。原来每条连接从 1 数起，
        // 重连以后的编号和同一个会话里以前的命令撞上，核心按编号去重，交回了那条旧命令的回应，什么都没做。
        let first: Vec<String> = (0..3).map(|_| super::next_id()).collect();
        let after_reconnect: Vec<String> = (0..3).map(|_| super::next_id()).collect();
        assert!(
            first.iter().all(|id| !after_reconnect.contains(id)),
            "{first:?} {after_reconnect:?}"
        );
        assert!(first.iter().chain(&after_reconnect).all(|id| owns(id)));
    }

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

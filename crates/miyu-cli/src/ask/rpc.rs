//! 在一条连接上说 JSON-RPC 2.0，一行一条（`docs/designs/04-核心协议.md` P1）：发请求；回应和推送由另一个任务
//! 读进来，照先后排着。等回应时来的推送留着，之后照先后交出去，一条都不丢。
//!
//! 请求的编号就是命令的编号，同一个编号只生效一次（`02-内核.md` 不变量 9，那是给断线重发的）：每条连接取一段
//! 随机数当前缀，两次 `miyu ask` 说给同一个会话的话不会撞号（施工中查出的：原先都从 `ask-1` 数起，第二次
//! `--continue` 说的那一句被当成了重发）。

use std::collections::VecDeque;
use std::io;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::sync::mpsc;

use miyu_ipc::Connection;

/// 连着核心的一头。
pub(crate) struct Rpc {
    /// 写的一头。
    writer: WriteHalf<Connection>,
    /// 读进来的回应和推送，照先后。
    incoming: mpsc::UnboundedReceiver<Value>,
    /// 等回应时来的推送。
    held: VecDeque<Value>,
    /// 这条连接的编号前缀：一段随机数。
    prefix: String,
    /// 下一条请求的序号。
    next: u64,
}

impl Rpc {
    /// 在 `connection` 上说话：另起一个读的任务。
    pub(crate) fn new(connection: Connection) -> Rpc {
        let (reader, writer) = tokio::io::split(connection);
        let (sender, incoming) = mpsc::unbounded_channel();
        tokio::spawn(read_all(BufReader::new(reader), sender));
        Rpc {
            writer,
            incoming,
            held: VecDeque::new(),
            prefix: prefix(),
            next: 0,
        }
    }

    /// 发一条请求，交回它的编号，也就是这条命令的编号。
    ///
    /// # Errors
    ///
    /// 写不出去。
    pub(crate) async fn send(&mut self, method: &str, params: Value) -> io::Result<String> {
        self.next += 1;
        let id = format!("ask-{}-{}", self.prefix, self.next);
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.writer
            .write_all(format!("{request}\n").as_bytes())
            .await?;
        Ok(id)
    }

    /// 发一条请求，等到它的回应。核心断开了是 `None`。
    ///
    /// # Errors
    ///
    /// 写不出去。
    pub(crate) async fn call(&mut self, method: &str, params: Value) -> io::Result<Option<Value>> {
        let id = self.send(method, params).await?;
        while let Some(message) = self.incoming.recv().await {
            if message["id"] == json!(id) {
                return Ok(Some(message));
            }
            self.held.push_back(message);
        }
        Ok(None)
    }

    /// 下一条回应或推送，先交留着的。核心断开了是 `None`。
    pub(crate) async fn next(&mut self) -> Option<Value> {
        match self.held.pop_front() {
            Some(message) => Some(message),
            None => self.incoming.recv().await,
        }
    }
}

/// 编号前缀：8 个随机字节写成 16 位十六进制；取不到随机数的，用进程号和此刻的纳秒。
fn prefix() -> String {
    let mut bytes = [0u8; 8];
    match getrandom::fill(&mut bytes) {
        Ok(()) => bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        Err(_) => {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |since| since.as_nanos());
            format!("{:x}{nanos:x}", std::process::id())
        }
    }
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

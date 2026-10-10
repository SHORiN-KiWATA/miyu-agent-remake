//! 在连接上说 JSON-RPC（`docs/blueprint/protocol.md`）：连上、握手，发一条等它的回应，等会话的一轮结束。
//!
//! 一次只做一件事，推送先攒着：等回应时读到的推送放进队里，等一轮结束时先看队里的。量尺一条一条地说，用不着
//! 读写分开的任务。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};

use miyu_ipc::Connection;
use miyu_store::root::DataRoot;

/// 下一个请求的编号。整个进程里不重复：核心把同一个编号的重发当成同一个命令，造会话交回上一次造的那一个、说话不再
/// 开一轮（`protocol.md`「请求」），连接换了、核心重启了都照认。
static NEXT: AtomicU64 = AtomicU64::new(1);

/// 每一次等，最多等多久：长会话上一轮也就几百毫秒，等这么久还没有就是坏了。
pub const PATIENCE: Duration = Duration::from_secs(120);

/// 一个握过手的连接。
pub struct Rpc {
    read: BufReader<ReadHalf<Connection>>,
    write: WriteHalf<Connection>,
    pushes: VecDeque<Value>,
}

impl Rpc {
    /// 连上 `root` 的核心、握手。
    ///
    /// # Errors
    ///
    /// 连不上、握手被拒、等太久。
    pub async fn connect(root: &DataRoot) -> Result<Rpc, String> {
        let (connection, token) = miyu_ipc::connect(root)
            .await
            .map_err(|e| format!("连不上核心：{e}"))?;
        let (read, write) = tokio::io::split(connection);
        let mut rpc = Rpc {
            read: BufReader::new(read),
            write,
            pushes: VecDeque::new(),
        };
        let hello = json!({"protocol": [1, 1], "head": {"kind": "perf", "version": "0.0.0"},
            "caps": {"input": false}, "token": token});
        rpc.call("hello", hello).await?;
        Ok(rpc)
    }

    /// 发一条，交回它的 `result`。
    ///
    /// # Errors
    ///
    /// 写不进、读不到、回了错、等太久。
    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = next_id();
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.write
            .write_all(format!("{request}\n").as_bytes())
            .await
            .map_err(|e| format!("{method} 写不进：{e}"))?;
        loop {
            let line = self.line(method).await?;
            if line["id"] != json!(id) {
                self.pushes.push_back(line);
                continue;
            }
            if let Some(error) = line.get("error") {
                return Err(format!("{method} 回了错：{error}"));
            }
            return Ok(line["result"].clone());
        }
    }

    /// 等 `session` 推来一条 `turn.ended`。
    ///
    /// # Errors
    ///
    /// 读不到、等太久。
    pub async fn turn_ended(&mut self, session: &str) -> Result<(), String> {
        let ended = |push: &Value| {
            push["method"] == "event"
                && push["params"]["session"] == session
                && push["params"]["event"]["kind"] == "turn.ended"
        };
        while let Some(push) = self.pushes.pop_front() {
            if ended(&push) {
                return Ok(());
            }
        }
        loop {
            if ended(&self.line("turn.ended").await?) {
                return Ok(());
            }
        }
    }

    /// 读一行，读成 JSON。
    async fn line(&mut self, waiting: &str) -> Result<Value, String> {
        let mut line = String::new();
        let read = tokio::time::timeout(PATIENCE, self.read.read_line(&mut line))
            .await
            .map_err(|_| format!("等 {waiting} 等了 {} 秒", PATIENCE.as_secs()))?
            .map_err(|e| format!("等 {waiting} 时读不到：{e}"))?;
        if read == 0 {
            return Err(format!("等 {waiting} 时核心断开了"));
        }
        serde_json::from_str(&line).map_err(|e| format!("核心写来的不是 JSON：{e}"))
    }
}

/// 下一个请求的编号：`perf-<n>`，整个进程里不重复（[`NEXT`]）。
fn next_id() -> String {
    format!("perf-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

#[cfg(test)]
mod tests;

//! 桥自己问核心（`/file`、`/blob` 经核心的 `fs.read`、`blob.get` 读，核心施工 W-6）：开一条核心连接、握手、一问一答。
//! 一个 HTTP 请求一条，回完就关；推送不会有（不订阅），读到的别的行跳过。

use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines, ReadHalf, WriteHalf};

/// 请求的编号：整个进程一个计数器，前面带进程号，跨连接、跨核心重启都不重复（核心照编号去重，不变式 9）。
static NEXT: AtomicU64 = AtomicU64::new(0);

/// 核心拒了：原因码（`data.reason`，没有的写 JSON-RPC 的 `code`）和原话；连接出错的原因码是 `bridge`。
#[derive(Debug)]
pub struct Refused {
    /// 原因码。
    pub reason: String,
    /// 原话。
    pub message: String,
}

impl Refused {
    fn bridge(message: impl Into<String>) -> Self {
        Self { reason: "bridge".to_string(), message: message.into() }
    }
}

/// 一条握过手的核心连接。
pub struct Core {
    lines: Lines<BufReader<ReadHalf<miyu_ipc::Connection>>>,
    writer: WriteHalf<miyu_ipc::Connection>,
}

impl Core {
    /// 连核心（没在跑的照 `MIYU_CORE_BIN` 拉起来，和页面的连接同一套）、握手（带本机令牌）。
    ///
    /// # Errors
    ///
    /// 连不上、握手被拒：交回一句话。
    pub async fn open() -> Result<Self, String> {
        let (connection, token) = crate::link::connect().await?;
        let (reader, writer) = tokio::io::split(connection);
        let mut core = Self { lines: BufReader::new(reader).lines(), writer };
        let hello = json!({"protocol": [1, 1], "head": {"kind": "web", "version": env!("CARGO_PKG_VERSION")}, "token": token, "caps": {"input": false}});
        core.call("hello", hello).await.map_err(|r| format!("握手被拒：{}", r.message))?;
        Ok(core)
    }

    /// 问一句，等它的回应。
    ///
    /// # Errors
    ///
    /// 核心拒了（照原因码）；连接断了、写不出去（原因码 `bridge`）。
    pub async fn call(&mut self, method: &str, params: Value) -> Result<Value, Refused> {
        let id = format!("bridge-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed) + 1);
        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string() + "\n";
        self.writer.write_all(line.as_bytes()).await.map_err(|e| Refused::bridge(format!("写给核心时出错：{e}")))?;
        loop {
            let got = self.lines.next_line().await.map_err(|e| Refused::bridge(format!("读核心时出错：{e}")))?;
            let Some(got) = got else { return Err(Refused::bridge("核心断开了")) };
            let Ok(message) = serde_json::from_str::<Value>(&got) else { continue };
            if message["id"] != json!(id) {
                continue;
            }
            if let Some(error) = message.get("error") {
                let reason = error["data"]["reason"].as_str().map_or_else(|| error["code"].to_string(), str::to_string);
                return Err(Refused { reason, message: error["message"].as_str().unwrap_or("").to_string() });
            }
            return Ok(message["result"].clone());
        }
    }
}

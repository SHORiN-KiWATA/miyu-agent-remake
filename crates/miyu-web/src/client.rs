//! 照终端的样子连着核心：一问一答（施工 W-9 起在 `open.rs`，施工 9-1 下挪出来：`serve` 起来时也要问核心配置）。

use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};

use miyu_ipc::Connection;
use miyu_store::root::DataRoot;

use crate::serve::CoreCommand;
use crate::texts::Language;

/// 照终端的样子连着的核心：一问一答。
pub(crate) struct Core {
    lines: BufReader<ReadHalf<Connection>>,
    write: WriteHalf<Connection>,
    pub(crate) language: Language,
}

impl Core {
    /// 连核心（没在跑就拉起来）、出示本机令牌握手。
    pub(crate) async fn connect(root: &DataRoot, core: &CoreCommand) -> Result<Core, String> {
        let (connection, token) = miyu_ipc::connect_or_start(root, || core())
            .await
            .map_err(|error| error.to_string())?;
        Core::shake(connection, token).await
    }

    /// 握手。
    async fn shake(connection: Connection, token: String) -> Result<Core, String> {
        let (read, write) = tokio::io::split(connection);
        let mut core = Core {
            lines: BufReader::new(read),
            write,
            language: Language::En,
        };
        let hello = json!({
            "protocol": [1, 1],
            "head": {"kind": "miyu-web", "version": env!("CARGO_PKG_VERSION")},
            "locale": locale(),
            "token": token,
        });
        let shaken = core.call("hello", "hello", hello).await?;
        core.language = Language::of(shaken["language"].as_str());
        Ok(core)
    }

    /// 连已经在跑的核心（不拉起）、出示本机令牌握手（施工 9-1 下：`serve` 起来时问配置，不为这个拉起核心）。
    pub(crate) async fn connect_running(root: &DataRoot) -> Result<Core, String> {
        let (connection, token) = miyu_ipc::connect(root)
            .await
            .map_err(|error| error.to_string())?;
        Core::shake(connection, token).await
    }

    /// 发一条请求、等它的回应：交回 `result`，拒绝的交回原话。
    pub(crate) async fn call(
        &mut self,
        id: &str,
        method: &str,
        params: Value,
    ) -> Result<Value, String> {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        let sent = async {
            self.write
                .write_all(format!("{request}\n").as_bytes())
                .await?;
            self.write.flush().await
        };
        sent.await.map_err(|error| error.to_string())?;
        loop {
            let mut line = String::new();
            let read =
                tokio::time::timeout(Duration::from_secs(30), self.lines.read_line(&mut line))
                    .await
                    .map_err(|_| "no answer".to_string())?
                    .map_err(|error| error.to_string())?;
            if read == 0 {
                return Err("core disconnected".to_string());
            }
            let Ok(reply) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if reply["id"] != json!(id) {
                continue;
            }
            return match reply.get("result") {
                Some(result) => Ok(result.clone()),
                None => Err(reply["error"]["message"]
                    .as_str()
                    .unwrap_or("refused")
                    .to_string()),
            };
        }
    }
}

/// 这台机器的语言：`LC_ALL`、`LC_MESSAGES`、`LANG` 照先后（核心照它算 `auto` 的语言）。
fn locale() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

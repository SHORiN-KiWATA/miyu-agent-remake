//! 带一次性码开浏览器的那两样（`web-ui.md`「怎么走」第二条第 3、4 款）：照终端的样子连核心（出示本机令牌）、一问一答，
//! 要 `account.setup_code`；用系统的办法把网址交给浏览器。网址怎么拼、什么时候带码、印什么，各家照自己的命令定。

use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};

use miyu_ipc::Connection;
use miyu_store::root::DataRoot;

use crate::CoreCommand;

/// 怎么把网址交给浏览器：交出去了的是真。
pub trait Browser {
    /// 打开 `url`。
    fn open(&self, url: &str) -> bool;
}

/// 系统的办法：Linux 是 `xdg-open`，macOS 是 `open`，Windows 是 `cmd /C start "" <网址>`。
pub struct SystemBrowser;

impl Browser for SystemBrowser {
    fn open(&self, url: &str) -> bool {
        let mut command = if cfg!(target_os = "macos") {
            Command::new("open")
        } else if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/C", "start", ""]);
            command
        } else {
            Command::new("xdg-open")
        };
        command
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        command.status().is_ok_and(|status| status.success())
    }
}

/// 照终端的样子连着的核心：一问一答。
pub struct Core {
    lines: BufReader<ReadHalf<Connection>>,
    write: WriteHalf<Connection>,
    /// 握手回的 `language`。
    language: Option<String>,
}

impl Core {
    /// 连核心（没在跑就照 `core` 拉起来）、出示本机令牌握手，报自己是 `kind` 这个头。
    ///
    /// # Errors
    ///
    /// 连不上、拉不起来，握手被拒、没回：原话。
    pub async fn connect(root: &DataRoot, core: &CoreCommand, kind: &str) -> Result<Core, String> {
        let (connection, token) = miyu_ipc::connect_or_start(root, || core())
            .await
            .map_err(|error| error.to_string())?;
        Core::shake(connection, token, kind).await
    }

    /// 连已经在跑的核心（不拉起）、出示本机令牌握手，报自己是 `kind` 这个头（施工 9-1 下：网页软件的 `serve` 起来时问
    /// 配置，不为这个拉起核心）。
    ///
    /// # Errors
    ///
    /// 核心没在跑、连不上，握手被拒、没回：原话。
    pub async fn connect_running(root: &DataRoot, kind: &str) -> Result<Core, String> {
        let (connection, token) = miyu_ipc::connect(root)
            .await
            .map_err(|error| error.to_string())?;
        Core::shake(connection, token, kind).await
    }

    /// 在连上的 `connection` 上出示本机令牌 `token` 握手，报自己是 `kind`。
    async fn shake(connection: Connection, token: String, kind: &str) -> Result<Core, String> {
        let (read, write) = tokio::io::split(connection);
        let mut core = Core {
            lines: BufReader::new(read),
            write,
            language: None,
        };
        let hello = json!({
            "protocol": [1, 1],
            "head": {"kind": kind, "version": env!("CARGO_PKG_VERSION")},
            "locale": locale(),
            "token": token,
        });
        let shaken = core.call("hello", "hello", hello).await?;
        core.language = shaken["language"].as_str().map(str::to_string);
        Ok(core)
    }

    /// 握手回的 `language`：没回的是空的。
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// 发一条请求、等它的回应：交回 `result`，拒绝的交回原话。
    ///
    /// # Errors
    ///
    /// 写不出去、30 秒没回、核心断开：原因；核心拒了：它的原话。
    pub async fn call(&mut self, id: &str, method: &str, params: Value) -> Result<Value, String> {
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
pub fn locale() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

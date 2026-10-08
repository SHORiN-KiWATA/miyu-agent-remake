//! 扩展自己的配置的推送（施工 9-4 下下，`docs/blueprint/extensions.md`「配置」）：核心拉起的扩展握了手以后，盯着配置服务
//! 交给核心的那一份（配置文件改了、`secret.set` 换了值、手改了密钥文件，都会换上新的一份），把这个包自己的那份重算一遍，
//! 跟上一次交给它的比，变了推 `extension.config`（`{"keys": {键: 新值或 null}}`）。不用订阅；握手的回应写出去以后才起，
//! 回应以后的变化照样算（盯的那一头是握手时拿的）。连接断了跟着停。

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::extensions::config::{changes, own};

/// 推送的任务。
#[derive(Debug)]
pub(super) struct ExtensionConfigForwarder {
    task: JoinHandle<()>,
}

impl ExtensionConfigForwarder {
    /// 起一个：包是 `package`，握手时交出去的是 `handed`，`current` 是握手时拿的盯配置的那一头，写进 `out`。
    pub(super) fn start(
        package: String,
        handed: BTreeMap<String, Value>,
        current: watch::Receiver<Arc<Config>>,
        out: mpsc::Sender<String>,
    ) -> ExtensionConfigForwarder {
        ExtensionConfigForwarder {
            task: tokio::spawn(forward(package, handed, current, out)),
        }
    }
}

impl Drop for ExtensionConfigForwarder {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// 配置换了一份就重算，变了的推出去，记成交出去的那份。
async fn forward(
    package: String,
    mut handed: BTreeMap<String, Value>,
    mut current: watch::Receiver<Arc<Config>>,
    out: mpsc::Sender<String>,
) {
    while current.changed().await.is_ok() {
        let config = Arc::clone(&current.borrow_and_update());
        let now = own(&config, &package);
        let changed = changes(&handed, &now);
        if changed.is_empty() {
            continue;
        }
        tracing::debug!(target: "miyu::endpoint", package = package.as_str(), keys = changed.len(), "extension config pushed");
        let line =
            json!({"jsonrpc": "2.0", "method": "extension.config", "params": {"keys": changed}});
        if out.send(line.to_string()).await.is_err() {
            return;
        }
        handed = now;
    }
}

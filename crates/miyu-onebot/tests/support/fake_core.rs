//! 核心的替身（施工 O-16，O-28 下改）：用 `miyu-ipc` 的监听当核心，进程里的桥经内存里的管道连它（`pipe.rs` 补上本机令牌）。
//! 只答出示本机令牌的握手（回中文）和 `venue.sessions`（名下没有场所会话，施工 O-32：桥起来时等它的回应），别的请求不答，留着
//! 连接：桥登记工具、方法照样起来（写出去就接着走，「施工时定的」第 139、154 条）。原来还替 WebUI 验登录令牌、交一次性码、接
//! `/ws` 转来的，随桥自己的网页去掉。

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::task::JoinHandle;

use miyu_ipc::Connection;
use miyu_store::root::DataRoot;

/// 跑着的替身：放下就不再接新的连接。
pub struct FakeCore {
    accepting: JoinHandle<()>,
}

impl Drop for FakeCore {
    fn drop(&mut self) {
        self.accepting.abort();
    }
}

/// 在数据根 `root` 上起一个替身。
pub fn fake_core(root: &DataRoot) -> FakeCore {
    let dirs = miyu_ipc::Dirs {
        runtime_dir: None,
        ..miyu_ipc::Dirs::current()
    };
    let mut listener = miyu_ipc::open(root, &dirs).expect("起得来").listener;
    let accepting = tokio::spawn(async move {
        while let Ok(connection) = listener.accept().await {
            tokio::spawn(answer(connection));
        }
    });
    FakeCore { accepting }
}

/// 一条连接：第一行是出示本机令牌的握手就回接受，之后 `venue.sessions` 回一个都没有，别的读着不答，读到头为止。
async fn answer(connection: Connection) {
    let (read, mut write) = tokio::io::split(connection);
    let mut lines = BufReader::new(read);
    let mut first = String::new();
    if lines.read_line(&mut first).await.is_err() {
        return;
    }
    let request: Value = serde_json::from_str(&first).unwrap_or_default();
    if request["method"] != "hello" || !request["params"]["token"].is_string() {
        return;
    }
    let reply = json!({"jsonrpc": "2.0", "id": request["id"], "result": {"account": "admin", "language": "zh", "protocol": 1}});
    if write
        .write_all(format!("{reply}\n").as_bytes())
        .await
        .is_err()
    {
        return;
    }
    let mut rest = String::new();
    while lines.read_line(&mut rest).await.is_ok_and(|read| read > 0) {
        let request: Value = serde_json::from_str(&rest).unwrap_or_default();
        if request["method"] == "venue.sessions" {
            let reply = json!({"jsonrpc": "2.0", "id": request["id"], "result": {"sessions": []}});
            if write
                .write_all(format!("{reply}\n").as_bytes())
                .await
                .is_err()
            {
                return;
            }
        }
        rest.clear();
    }
}

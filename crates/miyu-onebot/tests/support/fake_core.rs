//! 核心的替身（施工 O-16）：WebUI 只认本机传输，测试照网页软件的样子用 `miyu-ipc` 的监听当核心，看得到每一行。
//!
//! - 出示本机令牌握手的（桥自己那一条、`miyu-onebot web` 那一条）：回中文；之后只答 `account.setup_code`（还没设过密码
//!   照 [`FakeCore::first`]），别的不答，留着连接。
//! - 出示登录令牌握手的（`/status` 验令牌）：数一次；是 [`LOGIN`] 的回接受，别的回 `bad_login`。
//! - 别的（`/ws` 转来的浏览器）：读到第一行，连同连接交给测试。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use miyu_ipc::Connection;
use miyu_store::root::DataRoot;

/// 核心认的登录令牌。
pub const LOGIN: &str = "5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e5c1e";

/// `account.setup_code` 交出的一次性码。
pub const CODE: &str = "9f03b21c9f03b21c9f03b21c9f03b21c9f03b21c9f03b21c9f03b21c9f03b21c";

/// `/ws` 转来的一条：读到的第一行（带换行），和这条连接的两头。
pub struct Forwarded {
    pub first: String,
    pub lines: BufReader<ReadHalf<Connection>>,
    pub write: WriteHalf<Connection>,
}

/// 跑着的替身。
pub struct FakeCore {
    /// `/ws` 转来的，照先后。
    pub forwarded: mpsc::UnboundedReceiver<Forwarded>,
    /// 出示登录令牌握手了几次。
    pub logins: Arc<AtomicUsize>,
    /// `account.setup_code` 回的 `first`：还没设过密码。出厂是真。
    pub first: Arc<AtomicBool>,
    accepting: JoinHandle<()>,
}

impl FakeCore {
    /// 出示登录令牌握手了几次。
    pub fn logins(&self) -> usize {
        self.logins.load(Ordering::SeqCst)
    }

    /// 不再接新的连接（监听跟着放下）：之后连核心的都连不上。
    pub fn close(&self) {
        self.accepting.abort();
    }
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
    let (sender, forwarded) = mpsc::unbounded_channel();
    let logins = Arc::new(AtomicUsize::new(0));
    let first = Arc::new(AtomicBool::new(true));
    let counted = Arc::clone(&logins);
    let unset = Arc::clone(&first);
    let accepting = tokio::spawn(async move {
        while let Ok(connection) = listener.accept().await {
            let state = (Arc::clone(&counted), Arc::clone(&unset));
            tokio::spawn(answer(connection, sender.clone(), state));
        }
    });
    FakeCore {
        forwarded,
        logins,
        first,
        accepting,
    }
}

/// 一条连接：看第一行是谁。
async fn answer(
    connection: Connection,
    forwarded: mpsc::UnboundedSender<Forwarded>,
    (logins, unset): (Arc<AtomicUsize>, Arc<AtomicBool>),
) {
    let (read, mut write) = tokio::io::split(connection);
    let mut lines = BufReader::new(read);
    let mut first = String::new();
    if lines.read_line(&mut first).await.is_err() {
        return;
    }
    let request: Value = serde_json::from_str(&first).unwrap_or_default();
    let params = &request["params"];
    let reply = if request["method"] == "hello" && params["token"].is_string() {
        json!({"jsonrpc": "2.0", "id": request["id"], "result": {"account": "admin", "language": "zh", "protocol": 1}})
    } else if request["method"] == "hello" && params["login"].is_string() {
        logins.fetch_add(1, Ordering::SeqCst);
        match params["login"] == LOGIN {
            true => {
                json!({"jsonrpc": "2.0", "id": request["id"], "result": {"account": "admin", "language": "zh", "protocol": 1}})
            }
            false => json!({"jsonrpc": "2.0", "id": request["id"], "error": {
                "code": -32010, "message": "登录过期了", "data": {"reason": "bad_login"}
            }}),
        }
    } else {
        if forwarded
            .send(Forwarded {
                first,
                lines,
                write,
            })
            .is_err()
        {
            // 测试已经不看了。
        }
        return;
    };
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
        rest.clear();
        if request["method"] != "account.setup_code" {
            continue;
        }
        let issued = json!({"code": CODE, "expires": "2026-10-07T00:05:00.000Z", "first": unset.load(Ordering::SeqCst)});
        let reply = json!({"jsonrpc": "2.0", "id": request["id"], "result": issued});
        if write
            .write_all(format!("{reply}\n").as_bytes())
            .await
            .is_err()
        {
            return;
        }
    }
}

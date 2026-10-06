//! 核心的替身（施工 W-10）：照登录令牌握手，答 `blob.get`、`fs.read`。每一问单开一个任务答，慢的 blob 晚一点答：同一条连接上
//! 的回应会乱序到，网页软件要照编号分回去。

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use miyu_ipc::Listener;

/// 认的登录令牌。
pub const LOGIN: &str = "c0ffee";

/// 一块最多问多少。
pub const CHUNK: usize = 512 * 1024;

/// 替身的状态：有哪些东西、问过几次、作废了没有。
#[derive(Default)]
pub struct Core {
    /// blob 的哈希 → 内容。
    pub blobs: BTreeMap<String, Vec<u8>>,
    /// 本机文件的绝对路径 → 内容。
    pub files: BTreeMap<String, Vec<u8>>,
    /// 每一问的 `(offset, length)`。
    pub reads: Mutex<Vec<(u64, u64)>>,
    /// 握手了几次。
    pub hellos: AtomicUsize,
    /// 登录令牌作废了：握手拒，连着的下一问就断开。
    pub revoked: AtomicBool,
    /// 每一块晚多少毫秒答（`sha256:slow` 另晚 300 毫秒）。
    pub delay_ms: AtomicUsize,
    /// 多给：比要的多给几个字节（网页软件只照要的写）。
    pub overfill: AtomicBool,
}

impl Core {
    /// 一个 blob。
    pub fn blob(mut self, hash: &str, bytes: Vec<u8>) -> Core {
        self.blobs.insert(hash.to_string(), bytes);
        self
    }

    /// 一份本机文件。
    pub fn file(mut self, path: &str, bytes: Vec<u8>) -> Core {
        self.files.insert(path.to_string(), bytes);
        self
    }
}

/// 接着连接答，直到测试结束。
pub fn serve(mut listener: Listener, core: Arc<Core>) {
    tokio::spawn(async move {
        while let Ok(connection) = listener.accept().await {
            let core = Arc::clone(&core);
            tokio::spawn(async move {
                let (half, write) = tokio::io::split(connection);
                let write = Arc::new(tokio::sync::Mutex::new(write));
                let mut lines = BufReader::new(half);
                let mut line = String::new();
                let mut shaken = false;
                while lines.read_line(&mut line).await.is_ok_and(|read| read > 0) {
                    let request: Value = serde_json::from_str(&line).expect("是 JSON");
                    line.clear();
                    if shaken && core.revoked.load(Ordering::SeqCst) {
                        // 作废了：核心断开这条连接。
                        return;
                    }
                    if request["method"] == "hello" {
                        core.hellos.fetch_add(1, Ordering::SeqCst);
                        let good = request["params"]["login"] == LOGIN
                            && !core.revoked.load(Ordering::SeqCst)
                            && request["params"]["token"].is_null();
                        let reply = match good {
                            true => {
                                answer(&request, Ok(json!({"account": "admin", "protocol": 1})))
                            }
                            false => answer(&request, Err("bad_login")),
                        };
                        write_line(&write, &reply).await;
                        shaken = good;
                        continue;
                    }
                    let core = Arc::clone(&core);
                    let write = Arc::clone(&write);
                    tokio::spawn(async move {
                        let reply = answer(&request, read(&core, &request).await);
                        write_line(&write, &reply).await;
                    });
                }
            });
        }
    });
}

/// 答 `blob.get`、`fs.read`：照 `offset`、`length` 切一段。
async fn read(core: &Core, request: &Value) -> Result<Value, &'static str> {
    let params = &request["params"];
    let bytes = match request["method"].as_str() {
        Some("blob.get") => core
            .blobs
            .get(params["blob"].as_str().unwrap_or_default())
            .ok_or("unknown_blob")?,
        Some("fs.read") => {
            let path = params["path"].as_str().unwrap_or_default();
            if path.starts_with("/data-root/") {
                return Err("path_forbidden");
            }
            core.files.get(path).ok_or("path_unreadable")?
        }
        _ => return Err("unknown_method"),
    };
    let offset = params["offset"].as_u64().unwrap_or(0);
    let length = params["length"].as_u64().unwrap_or(CHUNK as u64);
    if length > CHUNK as u64 {
        return Err("bad_params");
    }
    core.reads.lock().expect("没崩").push((offset, length));
    let mut wait = core.delay_ms.load(Ordering::SeqCst) as u64;
    if params["blob"] == "sha256:slow" {
        wait += 300;
    }
    if wait > 0 && length > 0 {
        tokio::time::sleep(Duration::from_millis(wait)).await;
    }
    let size = bytes.len() as u64;
    let start = offset.min(size) as usize;
    let extra = if core.overfill.load(Ordering::SeqCst) {
        3
    } else {
        0
    };
    let end = (offset + length + extra).min(size) as usize;
    Ok(json!({"data": STANDARD.encode(&bytes[start..end]), "size": size}))
}

fn answer(request: &Value, result: Result<Value, &str>) -> Value {
    match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": request["id"], "result": result}),
        Err(reason) => json!({
            "jsonrpc": "2.0",
            "id": request["id"],
            "error": {"code": -32010, "message": reason, "data": {"reason": reason}},
        }),
    }
}

async fn write_line(
    write: &tokio::sync::Mutex<tokio::io::WriteHalf<miyu_ipc::Connection>>,
    reply: &Value,
) {
    let mut write = write.lock().await;
    if write
        .write_all(format!("{reply}\n").as_bytes())
        .await
        .is_err()
    {
        // 网页软件那头关了。
    }
}

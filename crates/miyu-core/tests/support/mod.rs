//! 几个测试共用的：临时的数据根、在套接字上说 JSON-RPC 的头、读磁盘上的会话日志。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};

use miyu_endpoint::Core;
use miyu_ipc::{Connection, Dirs, Opened};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::SessionId;
use miyu_session::Models;
use miyu_store::env::{Env, Platform};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

/// 一个用完就删的临时数据根，建好了骨架。
pub struct Home {
    dir: PathBuf,
    pub root: DataRoot,
}

impl Home {
    pub fn new() -> Home {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-core-{}-{n}", std::process::id()));
        let root = DataRoot::locate(&Env {
            platform: Platform::current(),
            miyu_home: Some(dir.clone().into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        })
        .expect("MIYU_HOME 是绝对路径");
        root.prepare().expect("临时目录里建得了骨架");
        Home { dir, root }
    }

    /// 在套接字上等连接：不用 `$XDG_RUNTIME_DIR`，套接字放在数据根的 `run/` 里。
    pub fn open(&self) -> Opened {
        let dirs = Dirs {
            runtime_dir: None,
            ..Dirs::current()
        };
        miyu_ipc::open(&self.root, &dirs).expect("起得来")
    }

    /// 一份核心：管理员 admin，请求模型照 `models`。
    pub fn core(&self, models: Arc<dyn Models>, token: &str) -> Arc<Core> {
        Arc::new(Core::new(
            self.root.clone(),
            ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")),
            models,
            miyu_core::admin(),
            token.to_string(),
        ))
    }

    /// 磁盘上会话 `session` 的日志，照先后。
    pub fn log(&self, session: &str) -> Vec<Event> {
        let session = SessionId::parse(session).expect("会话编号合写法");
        let dir = self.root.session_dir(&miyu_core::admin(), &session);
        SessionLog::open(&dir, SEGMENT_LIMIT).expect("日志打得开").1
    }

    /// 等到磁盘上会话 `session` 有了 `turn.ended`，交回它的日志。最多十秒。
    pub async fn until_turn_ends(&self, session: &str) -> Vec<Event> {
        within("这一轮结束", async {
            loop {
                let log = self.log(session);
                if log
                    .iter()
                    .any(|event| matches!(event.body, Body::TurnEnded(_)))
                {
                    return log;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
    }
}

impl Drop for Home {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// 等 `future`，最多十秒。
pub async fn within<T>(what: &str, future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .unwrap_or_else(|_| panic!("十秒内没等到{what}"))
}

/// 在套接字上连着核心的头：握过手了。
pub struct Head {
    reader: BufReader<ReadHalf<Connection>>,
    writer: WriteHalf<Connection>,
}

impl Head {
    /// 照 `run/socket`、`run/token` 连上，握手。
    pub async fn connect(root: &DataRoot) -> Head {
        let (connection, token) = within("连上", miyu_ipc::connect(root))
            .await
            .expect("连得上");
        let (read, write) = tokio::io::split(connection);
        let mut head = Head {
            reader: BufReader::new(read),
            writer: write,
        };
        let reply = head
            .call(
                "hello-1",
                "hello",
                json!({
                    "protocol": [1, 1],
                    "head": {"kind": "test", "version": "0.0.0"},
                    "caps": {"input": false},
                    "token": token,
                }),
            )
            .await;
        assert!(reply.get("error").is_none(), "握得了手：{reply}");
        head
    }

    /// 发一条请求，读到它的回应为止：中间的推送不要。
    pub async fn call(&mut self, id: &str, method: &str, params: Value) -> Value {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.writer
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("写得进");
        within("回应", async {
            loop {
                let mut line = String::new();
                let read = self.reader.read_line(&mut line).await.expect("读得了");
                assert!(read > 0, "核心断开了");
                let reply: Value = serde_json::from_str(&line).expect("是 JSON");
                if reply["id"] == json!(id) {
                    return reply;
                }
            }
        })
        .await
    }

    /// 造一个会话，交回它的编号。
    pub async fn create(&mut self) -> String {
        let reply = self
            .call("create-1", "session.create", json!({"cwd": "/work"}))
            .await;
        reply["result"]["session"]
            .as_str()
            .unwrap_or_else(|| panic!("应该造出会话：{reply}"))
            .to_string()
    }

    /// 说一句。
    pub async fn say(&mut self, session: &str, text: &str) {
        let reply = self
            .call(
                "say-1",
                "session.send",
                json!({"session": session, "text": text}),
            )
            .await;
        assert!(reply.get("error").is_none(), "说得出：{reply}");
    }

    /// 打断这一轮，排着的退回。
    pub async fn interrupt(&mut self, session: &str) {
        let reply = self
            .call(
                "interrupt-1",
                "session.interrupt",
                json!({"session": session, "queued": "return"}),
            )
            .await;
        assert!(reply.get("error").is_none(), "打断得了：{reply}");
    }
}

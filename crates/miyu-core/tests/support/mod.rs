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
use miyu_store::log::read_events;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

/// 源码树里的资源目录：测试跑在 `crates/miyu-core/` 下，往上两级就是仓库根。
pub fn resources() -> ResourceRoot {
    ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

/// 一个用完就删的临时数据根，建好了骨架。
pub struct Home {
    dir: PathBuf,
    pub root: DataRoot,
}

impl Home {
    pub fn new() -> Home {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("miyu-core-{}-{}-{n}", std::process::id(), stamp()));
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
        self.core_with(models, token, Catalog::default())
    }

    /// 同上，工具目录是 `tools`（施工 7-3）。
    pub fn core_with(&self, models: Arc<dyn Models>, token: &str, tools: Catalog) -> Arc<Core> {
        Arc::new(Core::new(
            self.root.clone(),
            resources(),
            models,
            tools,
            None,
            miyu_core::admin(),
            token.to_string(),
        ))
    }

    /// 同 [`Home::core_with`]，可选软件包登记的查询表换成 `queries`（施工 W-4，`packages.rs`）：默认开的几个包
    /// 不是这里测的，用得上真的那张表的测试照 `miyu_core::packages::register(&resources())` 造。
    pub fn core_with_queries(
        &self,
        models: Arc<dyn Models>,
        token: &str,
        queries: miyu_endpoint::queries::Queries,
    ) -> Arc<Core> {
        Arc::new(
            Core::new(
                self.root.clone(),
                resources(),
                models,
                Catalog::default(),
                None,
                miyu_core::admin(),
                token.to_string(),
            )
            .with_queries(queries),
        )
    }

    /// 磁盘上会话 `session` 的日志，照先后。只读：会话可能正在写，载入用的 `SessionLog::open` 会截掉正在写的那半行（施工 3-9 下在 macOS 的 CI 上撞到过：会话目录刚建、第一段还没有，它报没有这个会话）。还没写出第一条的当是空的。
    pub fn log(&self, session: &str) -> Vec<Event> {
        let session = SessionId::parse(session).expect("会话编号合写法");
        let dir = self.root.session_dir(&miyu_core::admin(), &session);
        read_events(&dir).unwrap_or_default()
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
        // 等得久一点：第一次画 mermaid 要扫系统的字体库，CI 的 Windows 机器上几个测试一起扫会过十秒。
        tokio::time::timeout(Duration::from_secs(60), async {
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
        .unwrap_or_else(|_| panic!("一分钟内没等到回应"))
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

/// 起名用的纳秒数：Windows 上进程号复用得快，前一个测试进程留下的、核心开着文件删不掉的目录会撞名（2026-10-01 CI 撞见）。
fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos())
}

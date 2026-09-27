//! 几个测试共用的：临时的数据根、造一份核心、在内存管道上连核心的客户端、等磁盘上的日志。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf};

use miyu_endpoint::{Core, serve};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{AccountId, SessionId};
use miyu_session::testkit::Script;
use miyu_store::env::{Env, Platform};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

/// 本机令牌。
pub const TOKEN: &str = "token-for-tests";

/// 一个用完就删的临时数据根，建好了骨架。
pub struct Home {
    dir: PathBuf,
    pub root: DataRoot,
}

impl Home {
    pub fn new() -> Home {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-endpoint-{}-{n}", std::process::id()));
        let env = Env {
            platform: Platform::current(),
            miyu_home: Some(dir.clone().into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        };
        let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
        root.prepare().expect("临时目录里建得了骨架");
        Home { dir, root }
    }

    /// 一份核心：管理员 alice，请求模型照 `script` 回。同一个数据根上造第二份，就像核心重启过。
    pub fn core(&self, script: &Script) -> Arc<Core> {
        Arc::new(Core::new(
            self.root.clone(),
            ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")),
            Arc::new(script.clone()),
            alice(),
            TOKEN.to_string(),
        ))
    }

    /// 磁盘上会话 `session` 的日志，照先后。
    pub fn log(&self, session: &str) -> Vec<Event> {
        let session = SessionId::parse(session).expect("会话编号合写法");
        let dir = self.root.session_dir(&alice(), &session);
        SessionLog::open(&dir, SEGMENT_LIMIT).expect("日志打得开").1
    }

    /// 等到磁盘上会话 `session` 说完了 `turns` 轮，最多十秒。
    pub async fn until_turns(&self, session: &str, turns: usize) {
        let ended = |log: &[Event]| {
            log.iter()
                .filter(|event| matches!(event.body, Body::TurnEnded(_)))
                .count()
        };
        let waited = tokio::time::timeout(Duration::from_secs(10), async {
            while ended(&self.log(session)) < turns {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(
            waited.is_ok(),
            "十秒内没说完 {turns} 轮：{}",
            ended(&self.log(session))
        );
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

pub fn alice() -> AccountId {
    AccountId::parse("alice").expect("账号合写法")
}

/// 在内存管道上连着核心的客户端。
pub struct Client {
    reader: BufReader<ReadHalf<DuplexStream>>,
    writer: Option<WriteHalf<DuplexStream>>,
}

impl Client {
    /// 连上 `core`：另一头交给 `serve`。
    pub fn connect(core: Arc<Core>) -> Client {
        let (near, far) = tokio::io::duplex(64 * 1024);
        tokio::spawn(serve(far, core));
        let (read, write) = tokio::io::split(near);
        Client {
            reader: BufReader::new(read),
            writer: Some(write),
        }
    }

    /// 拿走写的一头：写很长的东西时放到另一个任务里写，读的一头照样读。拿走以后不能再 [`Client::line`]。
    pub fn detach_writer(&mut self) -> WriteHalf<DuplexStream> {
        self.writer.take().expect("写的一头还在")
    }

    /// 写一行。
    pub async fn line(&mut self, line: &str) {
        self.writer
            .as_mut()
            .expect("写的一头还在")
            .write_all(format!("{line}\n").as_bytes())
            .await
            .expect("写得进");
    }

    /// 读下一行，认成 JSON；对方关了的是 `None`。最多等十秒。
    pub async fn next(&mut self) -> Option<Value> {
        let mut line = String::new();
        let read = tokio::time::timeout(Duration::from_secs(10), self.reader.read_line(&mut line))
            .await
            .expect("十秒内有回应")
            .expect("读得了");
        (read > 0).then(|| serde_json::from_str(&line).expect("回应是 JSON"))
    }

    /// 读下一行，最多等 `wait`：等不到的是 `None`（对方关了的也是）。
    pub async fn next_within(&mut self, wait: Duration) -> Option<Value> {
        let mut line = String::new();
        match tokio::time::timeout(wait, self.reader.read_line(&mut line)).await {
            Ok(Ok(read)) if read > 0 => Some(serde_json::from_str(&line).expect("是 JSON")),
            _ => None,
        }
    }

    /// 订阅会话 `session` 的事件流，交回回应。
    pub async fn subscribe(&mut self, id: &str, session: &str) -> Value {
        self.call(
            id,
            "subscribe",
            json!({"session": session, "stream": "events"}),
        )
        .await
    }

    /// 一直读，读到 `id` 的回应为止：交回回应之前读到的推送，和回应。
    pub async fn until_reply(&mut self, id: &str) -> (Vec<Value>, Value) {
        let mut pushed = Vec::new();
        loop {
            let next = self.next().await.expect("没断开");
            if next["id"] == json!(id) {
                return (pushed, next);
            }
            pushed.push(next);
        }
    }

    /// 一直读推送，读到会话 `session` 的回合结束为止，交回读到的。
    pub async fn until_turn_ends(&mut self, session: &str) -> Vec<Value> {
        let mut pushed = Vec::new();
        loop {
            let next = self.next().await.expect("没断开");
            let ended = next["params"]["session"] == json!(session)
                && next["params"]["event"]["kind"] == json!("turn.ended");
            pushed.push(next);
            if ended {
                return pushed;
            }
        }
    }

    /// 发一条请求，读它的回应。
    pub async fn call(&mut self, id: &str, method: &str, params: Value) -> Value {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.line(&request.to_string()).await;
        self.next().await.expect("有回应")
    }

    /// 握手：令牌对，中文，能输入。
    pub async fn hello(&mut self) -> Value {
        self.call(
            "hello-1",
            "hello",
            json!({
                "protocol": [1, 1],
                "head": {"kind": "test", "version": "0.0.0"},
                "locale": "zh-CN",
                "caps": {"input": true},
                "token": TOKEN,
            }),
        )
        .await
    }

    /// 握手：令牌对，不能让人输入（像不是终端时的 `miyu ask`）。
    pub async fn hello_without_input(&mut self) -> Value {
        self.call(
            "hello-1",
            "hello",
            json!({
                "protocol": [1, 1],
                "head": {"kind": "test", "version": "0.0.0"},
                "caps": {"input": false},
                "token": TOKEN,
            }),
        )
        .await
    }

    /// 说一句，交回回应。
    pub async fn say(&mut self, id: &str, session: &str, text: &str) -> Value {
        self.call(
            id,
            "session.send",
            json!({"session": session, "text": text}),
        )
        .await
    }

    /// 造一个会话，交回它的编号。
    pub async fn create(&mut self, id: &str, cwd: &str) -> String {
        let reply = self.call(id, "session.create", json!({"cwd": cwd})).await;
        reply["result"]["session"]
            .as_str()
            .unwrap_or_else(|| panic!("应该造出会话：{reply}"))
            .to_string()
    }
}

/// 等到 `done` 成立，最多十秒。
pub async fn until(what: &str, done: impl Fn() -> bool) {
    let waited = tokio::time::timeout(Duration::from_secs(10), async {
        while !done() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "十秒内没等到{what}");
}

/// 回应里的原因码；不是拒绝的是 `None`。
pub fn reason(reply: &Value) -> Option<&str> {
    reply["error"]["data"]["reason"].as_str()
}

/// 推送里的事件种类，照先后。
pub fn kinds(pushed: &[Value]) -> Vec<String> {
    pushed
        .iter()
        .filter(|push| push["method"] == json!("event"))
        .map(|push| {
            push["params"]["event"]["kind"]
                .as_str()
                .unwrap_or("?")
                .to_string()
        })
        .collect()
}

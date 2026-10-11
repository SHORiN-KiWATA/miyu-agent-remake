//! 几个测试共用的：临时的数据根、造一份核心、连核心的客户端（内存管道上，或者真的套接字上）、等磁盘上的日志。

#![allow(dead_code, reason = "几个测试各用其中一部分")]
pub mod deleting;
pub mod extensions;
pub mod login;
pub mod memories;
pub mod packaged;
pub mod providers;
mod pushes;
pub mod venues;
#[allow(unused_imports, reason = "几个测试各用其中一部分")]
pub use pushes::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

use miyu_endpoint::{Core, serve};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{AccountId, SessionId};
use miyu_session::testkit::Script;
use miyu_store::env::{Env, Platform};
use miyu_store::log::read_events;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

/// 本机令牌。
pub const TOKEN: &str = "token-for-tests";

/// 源码树里出厂的资源目录。
pub fn default_resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 一个用完就删的临时数据根，建好了骨架；另有一个在数据根外面的工作目录（施工 4-7 下：数据根里哪一级都不许写）。
pub struct Home {
    dir: PathBuf,
    pub root: DataRoot,
    pub work: PathBuf,
}

impl Home {
    pub fn new() -> Home {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "miyu-endpoint-{}-{}-{n}",
            std::process::id(),
            stamp()
        ));
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
        // 出厂不带人格（施工 P-4 下）：样本的软件工程师装进系统区那一层，测试照旧用 `engineer`。
        miyu_session::testkit::install_sample_persona(&root);
        let work = std::env::temp_dir().join(format!(
            "miyu-endpoint-work-{}-{}-{n}",
            std::process::id(),
            stamp()
        ));
        std::fs::create_dir_all(&work).expect("建得了工作目录");
        let work = std::fs::canonicalize(&work).expect("在");
        Home { dir, root, work }
    }

    /// 一份核心：管理员 alice，请求模型照 `script` 回。同一个数据根上造第二份，就像核心重启过。
    pub fn core(&self, script: &Script) -> Arc<Core> {
        self.core_with(script, TOKEN)
    }

    /// 一份核心，本机令牌是 `token`。
    pub fn core_with(&self, script: &Script, token: &str) -> Arc<Core> {
        self.core_with_tools(script, Catalog::default(), token)
    }

    /// 一份核心，工具目录是 `tools`（施工 4-1）。沙盒当能用（施工 5-4 上）：执行命令的都是假工具，不起助手。
    pub fn core_with_tools(&self, script: &Script, tools: Catalog, token: &str) -> Arc<Core> {
        Arc::new(self.core_full(script, tools, None, token).with_sandbox(
            miyu_sandbox::Availability::Usable(PathBuf::from("miyu-sandbox")),
        ))
    }

    /// 一份核心，请求模型的端口由 `models` 造，工具目录是 `tools`（施工 7-6：几个会话各照各的剧本回）。沙盒当能用。
    pub fn core_with_models(
        &self,
        models: Arc<dyn miyu_session::Models>,
        tools: Catalog,
    ) -> Arc<Core> {
        let core = Core::new(
            self.root.clone(),
            ResourceRoot::at(default_resources()),
            models,
            tools,
            None,
            alice(),
            TOKEN.to_string(),
        );
        Arc::new(
            core.with_sandbox(miyu_sandbox::Availability::Usable(PathBuf::from(
                "miyu-sandbox",
            ))),
        )
    }

    /// 一份核心，配置照磁盘上现在的几份读（施工 8-2）：清单是端点的两项加上 `log.level`，系统的家目录是 `home`，
    /// 带 `env` 的项照 `env` 读环境变量。
    pub fn core_configured(
        &self,
        script: &Script,
        home: Option<PathBuf>,
        env: &[(&str, &str)],
    ) -> Arc<Core> {
        self.core_with_items(script, home, env, &[])
    }

    /// 同 [`Home::core_configured`]，清单后面再加上 `extra` 这几项（施工 8-5：类型是密钥的项，出厂的清单里还没有）。
    pub fn core_with_items(
        &self,
        script: &Script,
        home: Option<PathBuf>,
        env: &[(&str, &str)],
        extra: &[miyu_config::Item],
    ) -> Arc<Core> {
        let items = [
            miyu_endpoint::settings::UiSettings::ITEMS,
            miyu_endpoint::settings::PermissionSettings::ITEMS,
            miyu_log::settings::LogSettings::ITEMS,
            extra,
        ]
        .concat();
        let config = miyu_endpoint::config::Config::load(
            &self.root,
            &alice(),
            home.as_deref(),
            items,
            miyu_endpoint::config::Environment::of(env),
        );
        Arc::new(
            self.core_full(script, Catalog::default(), home, TOKEN)
                .with_config(config),
        )
    }

    /// 写一份配置文件：`relative` 是相对数据根的路径（`system/config.toml`），目录没有的建上。
    pub fn write(&self, relative: &str, text: &str) {
        let path = self.root.path().join(relative);
        std::fs::create_dir_all(path.parent().expect("有上一级")).expect("建得了目录");
        std::fs::write(path, text).expect("写得进");
    }

    /// 一份核心，工具目录是 `tools`，沙盒用不了（施工 5-4 上）。
    pub fn core_without_sandbox(&self, script: &Script, tools: Catalog) -> Arc<Core> {
        Arc::new(self.core_full(script, tools, None, TOKEN))
    }

    /// 一份核心，工具目录是 `tools`，这台机器上的沙盒照 `sandbox`，沙盒的缓存放在 `cache` 下面、你的 cargo 目录是
    /// `cargo_home`（施工 5-4 下）。
    pub fn core_sandboxed(
        &self,
        script: &Script,
        tools: Catalog,
        sandbox: miyu_sandbox::Availability,
        cache: Option<(PathBuf, Option<PathBuf>)>,
    ) -> Arc<Core> {
        let core = self
            .core_full(script, tools, None, TOKEN)
            .with_sandbox(sandbox);
        Arc::new(match cache {
            Some((root, cargo_home)) => core.with_sandbox_cache(root, cargo_home),
            None => core,
        })
    }

    /// 一份核心，系统的家目录是 `home`（施工 4-3 下）。
    pub fn core_at_home(&self, script: &Script, home: PathBuf) -> Arc<Core> {
        Arc::new(self.core_full(script, Catalog::default(), Some(home), TOKEN))
    }

    /// 一份核心，连上以后最多等 `wait` 握手（施工 4-9 再补三上：测试里不用真等 10 秒）。
    pub fn core_waiting_hello(&self, script: &Script, wait: std::time::Duration) -> Arc<Core> {
        Arc::new(self.bare(script, default_resources()).with_hello_wait(wait))
    }

    /// 一份核心，资源目录是 `resources`（施工 4-9 再补三上：造一份坏了的）。
    pub fn core_with_resources(&self, script: &Script, resources: PathBuf) -> Arc<Core> {
        Arc::new(self.bare(script, resources))
    }

    /// 一份核心，`fs.find` 的 `fresh` 照 `fresh` 这个时长判断要不要重建清单（施工 W-2）：测试里设短的，不用真等
    /// 十秒。
    pub fn core_files_fresh(&self, script: &Script, fresh: Duration) -> Arc<Core> {
        Arc::new(
            self.bare(script, default_resources())
                .with_files_fresh(fresh),
        )
    }

    fn bare(&self, script: &Script, resources: PathBuf) -> Core {
        Core::new(
            self.root.clone(),
            ResourceRoot::at(resources),
            Arc::new(script.clone()),
            Catalog::default(),
            None,
            alice(),
            TOKEN.to_string(),
        )
    }

    pub fn core_full(
        &self,
        script: &Script,
        tools: Catalog,
        home: Option<PathBuf>,
        token: &str,
    ) -> Core {
        Core::new(
            self.root.clone(),
            ResourceRoot::at(default_resources()),
            Arc::new(script.clone()),
            tools,
            home,
            alice(),
            token.to_string(),
        )
    }

    /// 磁盘上会话 `session` 的日志，照先后。只读：会话可能正在写，载入用的 `SessionLog::open` 会截掉正在写的那半行（施工 3-9 下在 macOS 的 CI 上撞到过：会话目录刚建、第一段还没有，它报没有这个会话）。还没写出第一条的当是空的。
    pub fn log(&self, session: &str) -> Vec<Event> {
        let session = SessionId::parse(session).expect("会话编号合写法");
        let dir = self.root.session_dir(&alice(), &session);
        read_events(&dir).unwrap_or_default()
    }

    /// 等到磁盘上会话 `session` 说完了 `turns` 轮。
    pub async fn until_turns(&self, session: &str, turns: usize) {
        let ended = |log: &[Event]| {
            log.iter()
                .filter(|event| matches!(event.body, Body::TurnEnded(_)))
                .count()
        };
        let waited = tokio::time::timeout(Duration::from_secs(60), async {
            while ended(&self.log(session)) < turns {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await;
        assert!(
            waited.is_ok(),
            "六十秒内没说完 {turns} 轮：{}",
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
        let _ = std::fs::remove_dir_all(&self.work);
    }
}

pub fn alice() -> AccountId {
    AccountId::parse("alice").expect("账号合写法")
}

/// 写的一头。
pub type Writer = Box<dyn AsyncWrite + Unpin + Send>;

/// 连着核心的客户端。
pub struct Client {
    reader: BufReader<Box<dyn AsyncRead + Unpin + Send>>,
    writer: Option<Writer>,
}

impl Client {
    /// 在内存管道上连上 `core`：另一头交给 `serve`。
    pub fn connect(core: Arc<Core>) -> Client {
        let (near, far) = tokio::io::duplex(64 * 1024);
        tokio::spawn(serve(far, core));
        Client::over(near)
    }

    /// 在 `stream` 上说话：另一头已经连着核心了。
    pub fn over(stream: impl AsyncRead + AsyncWrite + Send + 'static) -> Client {
        let (read, write) = tokio::io::split(stream);
        Client {
            reader: BufReader::new(Box::new(read)),
            writer: Some(Box::new(write)),
        }
    }

    /// 拿走写的一头：写很长的东西时放到另一个任务里写，读的一头照样读。拿走以后不能再 [`Client::line`]。
    pub fn detach_writer(&mut self) -> Writer {
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

    /// 读下一行，认成 JSON；对方关了的是 `None`。最多等六十秒（原来十秒，机器忙时会超过，2026-10-07）。
    pub async fn next(&mut self) -> Option<Value> {
        let mut line = String::new();
        let read = tokio::time::timeout(Duration::from_secs(60), self.reader.read_line(&mut line))
            .await
            .expect("六十秒内有回应")
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

    /// 带 `after` 订阅会话 `session` 的事件流（施工 3-8 六补）：交回回应之前读到的推送（补的在里面），和回应。
    pub async fn subscribe_after(
        &mut self,
        id: &str,
        session: &str,
        after: Value,
    ) -> (Vec<Value>, Value) {
        let params = json!({"session": session, "stream": "events", "after": after});
        let request = json!({"jsonrpc": "2.0", "id": id, "method": "subscribe", "params": params});
        self.line(&request.to_string()).await;
        self.until_reply(id).await
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
        self.hello_as(TOKEN).await
    }

    /// 握手：出示 `token`，中文，能输入。
    pub async fn hello_as(&mut self, token: &str) -> Value {
        self.call(
            "hello-1",
            "hello",
            json!({
                "protocol": [1, 1],
                "head": {"kind": "test", "version": "0.0.0"},
                "locale": "zh-CN",
                "caps": {"input": true},
                "token": token,
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
        self.create_with(id, json!({"cwd": cwd})).await
    }

    /// 造一个人格是 `persona` 的会话（施工 P-4 上：出厂不设默认人格，要人格的测试明着写），交回编号。
    pub async fn create_as(&mut self, id: &str, cwd: &str, persona: &str) -> String {
        self.create_with(id, json!({"cwd": cwd, "persona": persona}))
            .await
    }

    /// 照参数 `params` 造一个会话，交回编号。
    pub async fn create_with(&mut self, id: &str, params: serde_json::Value) -> String {
        let reply = self.call(id, "session.create", params).await;
        reply["result"]["session"]
            .as_str()
            .unwrap_or_else(|| panic!("应该造出会话：{reply}"))
            .to_string()
    }
}

/// 等到 `done` 成立，最多六十秒。
pub async fn until(what: &str, done: impl Fn() -> bool) {
    let waited = tokio::time::timeout(Duration::from_secs(60), async {
        while !done() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(waited.is_ok(), "六十秒内没等到{what}");
}

/// 回应里的原因码；不是拒绝的是 `None`。
/// 订阅的回应去掉「当前的」几格（施工 9-6 上：`usage`、`permission`、`jobs`；施工 9-7 上：`workspace`）：只看限额、模型、补发
/// 这些的测试用，这几格另有 `tests/subscribe_now.rs`、`tests/limits.rs`、`tests/workspace_property.rs` 守着。
pub fn apart_from_now(result: &Value) -> Value {
    let mut result = result.clone();
    if let Some(fields) = result.as_object_mut() {
        for key in ["usage", "permission", "jobs", "workspace"] {
            fields.remove(key);
        }
    }
    result
}

pub fn reason(reply: &Value) -> Option<&str> {
    reply["error"]["data"]["reason"].as_str()
}

/// 起名用的纳秒数：Windows 上进程号复用得快，前一个测试进程留下的、核心开着文件删不掉的目录会撞名（2026-10-01 CI 撞见）。
fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos())
}

//! 桥的测试共用的（施工 O-8，`onebot.md`「守着它的」）：临时的数据根里起一个真的核心（请求模型照剧本回，系统配置里
//! `qq:10001` 是管理员本人），起一个桥（端口 0 让系统挑），假的 NapCat 是一个 WebSocket 客户端。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

pub mod napcat;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use miyu_config::secret::Secret;
use miyu_endpoint::Core;
use miyu_endpoint::config::{Config, Environment};
use miyu_kernel::id::{AccountId, SessionId};
use miyu_onebot::serve::{Failure, Notice, Serve, run};
use miyu_onebot::settings::Settings;
use miyu_onebot::tuning::Tuning;
use miyu_session::testkit::Script;
use miyu_store::env::{Env, Platform};
use miyu_store::log::read_events;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

#[allow(unused_imports, reason = "读配置的测试用不上假 NapCat")]
pub use napcat::*;

/// 主人的 QQ 号：系统配置里对着管理员。
pub const OWNER: i64 = 10001;

/// 不在对应表里的人。
pub const STRANGER: i64 = 20002;

/// 机器人的号。
pub const BOT: i64 = 30003;

/// 假 NapCat 发的私聊带的平台时刻（事件的 `time`，整数秒）：拼进命令编号的最后一段。
pub const TIME: i64 = 1_759_800_000;

/// 桥的访问令牌。
pub const TOKEN: &str = "napcat-test-token";

/// 系统配置：主人对应表。
const CONFIG: &str = "[external.bindings]\n\"qq:10001\" = \"admin\"\n";

/// 一个用完就删的临时数据根，里面跑着一个核心。
pub struct Home {
    dir: PathBuf,
    pub root: DataRoot,
    core: Arc<Core>,
    running: JoinHandle<std::convert::Infallible>,
}

/// 管理员的账号。
pub fn admin() -> AccountId {
    AccountId::parse("admin").expect("账号合写法")
}

/// 源码树里的资源目录。
pub fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 出厂的 `bridge.json`。
pub fn tuning() -> Tuning {
    Tuning::load(&resources()).expect("出厂的 bridge.json 读得出来")
}

/// 一个新的临时数据根（`MIYU_HOME` 指到它），建好骨架：交回目录（用完调的一方删）和数据根。
pub fn temp_root() -> (PathBuf, DataRoot) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let dir = std::env::temp_dir().join(format!(
        "miyu-onebot-{}-{nanos}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
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
    (dir, root)
}

impl Home {
    /// 起一个核心：请求模型照 `script`，没有工具，系统配置是主人对应表。
    pub fn new(script: &Script) -> Home {
        let (dir, root) = temp_root();
        let file = root.path().join("system").join("config.toml");
        std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了目录");
        std::fs::write(&file, CONFIG).expect("写得进");
        let dirs = miyu_ipc::Dirs {
            runtime_dir: None,
            ..miyu_ipc::Dirs::current()
        };
        let opened = miyu_ipc::open(&root, &dirs).expect("起得来");
        let config = Config::load(
            &root,
            &admin(),
            None,
            miyu_core::settings::items(),
            Environment::of(&[]),
        );
        let core = Arc::new(
            Core::new(
                root.clone(),
                ResourceRoot::at(resources()),
                Arc::new(script.clone()),
                Catalog::default(),
                None,
                admin(),
                opened.token.clone(),
            )
            .with_config(config),
        );
        let running = tokio::spawn(miyu_endpoint::run(opened.listener, Arc::clone(&core)));
        Home {
            dir,
            root,
            core,
            running,
        }
    }

    /// 管理员名下的会话（场所会话也在这里）。
    pub fn sessions(&self) -> Vec<SessionId> {
        self.root.sessions(&admin()).expect("列得出")
    }

    /// 会话 `session` 日志里的事件，写成 JSON。还没写出第一条的当是空的。
    pub fn events(&self, session: &SessionId) -> Vec<Value> {
        read_events(&self.root.session_dir(&admin(), session))
            .unwrap_or_default()
            .iter()
            .map(|event| serde_json::to_value(event).expect("写得成 JSON"))
            .collect()
    }

    /// 全部会话里人说的话（`message.user`），照会话、照先后。
    pub fn said(&self) -> Vec<Value> {
        self.sessions()
            .iter()
            .flat_map(|session| self.events(session))
            .filter(|event| event["kind"] == "message.user")
            .collect()
    }

    /// 全部会话里人说的话的文字。
    pub fn said_texts(&self) -> Vec<String> {
        self.said()
            .iter()
            .map(|event| {
                event["body"]["blocks"][0]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string()
            })
            .collect()
    }
}

impl Drop for Home {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        self.running.abort();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// 跑着的一个桥。
pub struct Bridge {
    /// 实际听的端口。
    pub port: u16,
    /// 它说给人听的，照先后。
    pub notices: Arc<Mutex<Vec<Notice>>>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<Result<(), Failure>>,
}

impl Bridge {
    /// 叫它停下，等它停完。
    pub async fn stop(mut self) -> Result<(), Failure> {
        if let Some(stop) = self.stop.take()
            && stop.send(()).is_err()
        {
            // 它已经自己停了。
        }
        within("桥停下", &mut self.task).await.expect("没崩")
    }
}

/// 拉不起来的核心：命令不存在。测试里核心已经在跑，用不上。
pub fn no_core() -> std::process::Command {
    std::process::Command::new("/nonexistent/miyu-core-for-tests")
}

/// 在 `home` 上起一个桥：端口 0，令牌是 [`TOKEN`]，等它说在哪个端口等 NapCat。
pub async fn bridge(home: &Home) -> Bridge {
    let notices = Arc::new(Mutex::new(Vec::new()));
    let (stop, stopped) = oneshot::channel::<()>();
    let (port_tx, port_rx) = oneshot::channel();
    let port_tx = Mutex::new(Some(port_tx));
    let heard = Arc::clone(&notices);
    let serve = Serve {
        root: home.root.clone(),
        settings: Settings {
            port: 0,
            token: Secret::new(TOKEN).expect("合写法"),
        },
        core: Arc::new(no_core),
        locale: Some("zh_CN.UTF-8".to_string()),
        tuning: tuning(),
    };
    let task = tokio::spawn(run(
        serve,
        move |notice| {
            if let Notice::Listening { port, .. } = &notice
                && let Some(port_tx) = port_tx.lock().expect("没 panic").take()
                && port_tx.send(*port).is_err()
            {
                // 等的那头已经不在了。
            }
            heard.lock().expect("没 panic").push(notice);
        },
        async move {
            if stopped.await.is_err() {
                // 测试丢了停的那一头：照样停。
            }
        },
    ));
    let port = within("桥开始听", port_rx).await.expect("说了端口");
    Bridge {
        port,
        notices,
        stop: Some(stop),
        task,
    }
}

/// 等 `future`，最多十秒。
pub async fn within<T>(what: &str, future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .unwrap_or_else(|_| panic!("十秒内没等到{what}"))
}

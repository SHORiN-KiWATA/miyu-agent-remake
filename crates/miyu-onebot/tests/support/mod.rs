//! 桥的测试共用的（施工 O-8，`onebot.md`「守着它的」）：临时的数据根里起一个真的核心（请求模型照剧本回，系统配置里
//! `qq:10001` 是管理员本人），起一个桥（端口 0 让系统挑），假的 NapCat 是一个 WebSocket 客户端。在进程里跑的桥经内存里的
//! 管道连核心（`pipe`，施工 O-18）；真核心拉起真桥、跑真的程序的在 `spawning`。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

pub mod fake_core;
pub mod http;
pub mod napcat;
pub mod pipe;
pub mod spawning;

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
use miyu_endpoint::extensions::Timing;
use miyu_kernel::id::{AccountId, SessionId};
use miyu_onebot::serve::{Failure, Notice, Serve, run};
use miyu_onebot::settings::{Reload, Settings, Token, load};
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
        Home::with_config(script, CONFIG, None)
    }

    /// 起一个照开关拉起扩展的核心（施工 O-18）：系统配置是主人对应表接着 `more`（端口、令牌、语言），退避从 20 毫秒起、
    /// 最多 100 毫秒，请扩展退出以后照出厂的等 5 秒再杀（等的时候桥得自己退）。出厂的清单里有桥：开了就拉起测试程序旁边的
    /// `miyu-onebot`（[`spawning::linked`]）。
    pub fn spawning(script: &Script, more: &str) -> Home {
        spawning::linked();
        let timing = Timing {
            grace: Duration::from_secs(5),
            stable: Duration::from_secs(60),
            backoff: Duration::from_millis(20),
            longest: Duration::from_millis(100),
        };
        Home::with_config(script, &format!("{CONFIG}{more}"), Some(timing))
    }

    /// 起一个核心：系统配置写成 `config`；`extensions` 有的照它等、退避，照开关拉起扩展。
    fn with_config(script: &Script, config: &str, extensions: Option<Timing>) -> Home {
        let (dir, root) = temp_root();
        let file = root.path().join("system").join("config.toml");
        std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了目录");
        std::fs::write(&file, config).expect("写得进");
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
            .with_config(config)
            .with_extension_timing(extensions.unwrap_or_default()),
        );
        if extensions.is_some() {
            core.start_extensions();
        }
        let running = tokio::spawn(miyu_endpoint::run(opened.listener, Arc::clone(&core)));
        Home {
            dir,
            root,
            core,
            running,
        }
    }

    /// 请核心拉起的扩展都退出，等它们退出（施工 O-18）。
    pub async fn stop_extensions(&self) {
        self.core.stop_extensions().await;
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
    /// NapCat 连进来的端口，实际听的那一个。
    pub port: u16,
    /// WebUI 实际听的端口。
    pub web: u16,
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

    /// 不叫它停，等它自己停下（施工 O-18：核心关了管道）。
    pub async fn ended(mut self) -> Result<(), Failure> {
        within("桥自己停下", &mut self.task).await.expect("没崩")
    }
}

/// 拉不起来的核心：命令不存在。测试里核心已经在跑，用不上（WebUI、`start` 这几样连核心时才要）。
pub fn no_core() -> std::process::Command {
    std::process::Command::new("/nonexistent/miyu-core-for-tests")
}

/// 两个端口都是 0（让系统挑），令牌是 [`TOKEN`]。
pub fn settings() -> Settings {
    with_token(Some(TOKEN))
}

/// 两个端口都是 0，令牌是 `token`；空的是没写引用（施工 O-16 补二：令牌读成三种）。
pub fn with_token(token: Option<&str>) -> Settings {
    Settings {
        port: 0,
        web: 0,
        token: token.map_or(Token::Unset, |token| {
            Token::Set(Secret::new(token).expect("合写法"))
        }),
    }
}

/// 两个端口都是 0，令牌写了引用、取不到。
pub fn missing_token() -> Settings {
    Settings {
        port: 0,
        web: 0,
        token: Token::Missing,
    }
}

/// 令牌没设的那一句（跟在说在哪等 NapCat 的后面）。
pub fn no_token() -> Notice {
    Notice::NoToken
}

/// NapCat 的令牌对不上时，隔 `seconds` 秒才重读一次配置。
pub fn reload_every(tuning: &mut Tuning, seconds: u64) {
    tuning.reload_seconds = seconds;
}

/// 照磁盘上的配置重读（真的 `load`）：数据根是 `root`，环境是空的。
pub fn from_disk(root: &DataRoot) -> Reload {
    let root = root.clone();
    Arc::new(move || load(&root, None, None, Environment::of(&[])).settings)
}

/// 照页面的办法写令牌（施工 O-16 补二）：经核心 `secret.set` 存成 `onebot`，再 `config.set` 把 `onebot.token` 写成引用它。
/// 核心要已经在 `root` 上跑着。
pub async fn set_token(root: &DataRoot, value: &str) {
    let mut core = within(
        "连上核心",
        miyu_webserve::open::Core::connect_running(root, "test"),
    )
    .await
    .expect("连得上核心");
    core.call(
        "secret",
        "secret.set",
        serde_json::json!({"name": "onebot", "value": value}),
    )
    .await
    .expect("存得进");
    core.call(
        "config",
        "config.set",
        serde_json::json!({"layer": "system", "changes": [{"key": "onebot.token", "value": {"secret": "onebot"}}]}),
    )
    .await
    .expect("写得进");
}

/// 重读配置读到的总是 `settings`。
pub fn same(settings: Settings) -> Reload {
    Arc::new(move || Ok(settings.clone()))
}

/// 在数据根 `root` 上起一个桥要的：设置照 `settings`，经内存里的管道连 `root` 上的那个核心（[`pipe::pipe_to`]，施工 O-18），
/// WebUI 拉不起核心，说中文，出厂的 `bridge.json` 和资源目录，重读配置读到的和起来时一样。
pub fn serve(root: DataRoot, settings: Settings) -> Serve {
    Serve {
        pipe: pipe::pipe_to(&root),
        root,
        settings: settings.clone(),
        core: Arc::new(no_core),
        locale: Some("zh_CN.UTF-8".to_string()),
        tuning: tuning(),
        resources: ResourceRoot::at(resources()),
        reload: same(settings),
    }
}

/// 在 `home` 上起一个桥：端口 0，令牌是 [`TOKEN`]，等它说在哪两个端口听。
pub async fn bridge(home: &Home) -> Bridge {
    start(serve(home.root.clone(), settings())).await
}

/// 照 `serve` 起一个桥，等它说在哪两个端口听（令牌没设的只说 WebUI 的，NapCat 的端口照设的那一个）。
pub async fn start(serve: Serve) -> Bridge {
    let notices = Arc::new(Mutex::new(Vec::new()));
    let (stop, stopped) = oneshot::channel::<()>();
    let (told, mut telling) = tokio::sync::mpsc::unbounded_channel();
    let heard = Arc::clone(&notices);
    let mut port = serve.settings.port;
    let task = tokio::spawn(run(
        serve,
        move |notice| {
            if told.send(notice.clone()).is_err() {
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
    // 说在哪等 NapCat 的那一句在 WebUI 那一句前面。
    let web = within("WebUI 开始听", async {
        loop {
            match telling.recv().await.expect("说了端口") {
                Notice::Listening { port: heard, .. } => port = heard,
                Notice::Web { port } => return port,
                _ => {}
            }
        }
    })
    .await;
    Bridge {
        port,
        web,
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

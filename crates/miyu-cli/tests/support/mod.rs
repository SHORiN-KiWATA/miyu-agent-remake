//! 几个测试共用的：临时的数据根、在进程里起一个核心（请求模型照剧本或者没有 key，工具照给的目录）、在真的
//! 套接字上跑一遍 `talk`、读会话日志；数据根外面的临时目录；没有核心的数据根，测试自己在套接字上当核心。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;

use miyu_cli::language::Language;
use miyu_cli::{CompactPlan, Format, Plan, Screen, Target, UndoPlan, compact_on, talk, undo_on};
use miyu_endpoint::Core;
use miyu_ipc::Dirs;
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::SessionId;
use miyu_sandbox::{Availability, Unusable};
use miyu_session::Models;
use miyu_store::env::{Env, Platform};
use miyu_store::human::Human;
use miyu_store::log::read_events;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

/// 一个用完就删的临时数据根，里面跑着一个核心。
pub struct Home {
    dir: PathBuf,
    pub root: DataRoot,
    running: tokio::task::JoinHandle<std::convert::Infallible>,
}

/// 源码树里的资源目录。
pub fn resources() -> ResourceRoot {
    ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
}

/// 这次测试里第几个临时目录：同一个进程里不撞名。
fn next() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

impl Home {
    /// 起一个核心：请求模型照 `models`，没有工具。沙盒当能用：没有工具，助手用不上，随便一条路径。
    pub fn new(models: Arc<dyn Models>) -> Home {
        Home::start(
            models,
            Catalog::default(),
            Availability::Usable(PathBuf::from("miyu-sandbox")),
        )
    }

    /// 起一个核心：请求模型照 `models`，工具照 `tools`。沙盒能用，助手是 cargo 编出来的那一个（施工 5-4 上）：
    /// `miyu ask` 里执行命令不问人。
    pub fn with_tools(models: Arc<dyn Models>, tools: Catalog) -> Home {
        Home::start(
            models,
            tools,
            Availability::Usable(miyu_sandbox::testkit::built_helper()),
        )
    }

    /// 起一个核心：请求模型照 `models`，没有工具，沙盒用不了，原因是 `reason`（施工 5-4 下）。
    pub fn without_sandbox(models: Arc<dyn Models>, reason: Unusable) -> Home {
        Home::start(models, Catalog::default(), Availability::Unusable(reason))
    }

    fn start(models: Arc<dyn Models>, tools: Catalog, sandbox: Availability) -> Home {
        let (dir, root) = temp_root();
        let opened = miyu_ipc::open(&root, &dirs()).expect("起得来");
        let core = Arc::new(
            Core::new(
                root.clone(),
                resources(),
                models,
                tools,
                None,
                AccountIdOf::admin(),
                opened.token.clone(),
            )
            .with_sandbox(sandbox),
        );
        let running = tokio::spawn(miyu_endpoint::run(opened.listener, core));
        Home { dir, root, running }
    }

    /// 在真的套接字上连上核心，照 `plan` 说一句。`presses` 是 Ctrl+C。
    pub async fn ask_with(&self, plan: &Plan, presses: mpsc::Receiver<()>) -> Asked {
        ask_at(&self.root, plan, presses).await
    }

    /// 在真的套接字上连上核心，照 `plan` 撤一次（恢复一次，施工 4-7 下）。
    pub async fn undo(&self, plan: &UndoPlan) -> Asked {
        let (connection, token) = miyu_ipc::connect(&self.root).await.expect("连得上");
        let tape = Tape::default();
        let (mut out, mut err) = (tape.pen(false), tape.pen(true));
        let code = within(
            "撤完",
            undo_on(connection, &token, plan, &mut out, &mut err),
        )
        .await;
        Asked {
            code,
            out: tape.text(|err| !err),
            err: tape.text(|err| err),
            screen: tape.text(|_| true),
        }
    }

    /// 在真的套接字上连上核心，照 `plan` 手动压缩一次（施工 6-8），不按 Ctrl+C。
    pub async fn compact(&self, plan: &CompactPlan) -> Asked {
        let (_press, presses) = mpsc::channel(1);
        self.compact_with(plan, presses).await
    }

    /// 同 [`Home::compact`]，`presses` 是 Ctrl+C。
    pub async fn compact_with(&self, plan: &CompactPlan, presses: mpsc::Receiver<()>) -> Asked {
        let (connection, token) = miyu_ipc::connect(&self.root).await.expect("连得上");
        let tape = Tape::default();
        let (mut out, mut err) = (tape.pen(false), tape.pen(true));
        let mut screen = Screen {
            out: &mut out,
            err: &mut err,
            gray: false,
            live: false,
        };
        let code = within(
            "压完",
            compact_on(connection, &token, plan, &mut screen, presses),
        )
        .await;
        Asked {
            code,
            out: tape.text(|err| !err),
            err: tape.text(|err| err),
            screen: tape.text(|_| true),
        }
    }

    /// 照 `plan` 说一句，不按 Ctrl+C。
    pub async fn ask(&self, plan: &Plan) -> Asked {
        let (_press, presses) = mpsc::channel(1);
        self.ask_with(plan, presses).await
    }

    /// 在协议上直接造一个普通的（不是一次性的）会话，交回编号：好看 `--continue` 会不会跳过它。
    pub async fn create_plain(&self) -> String {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (connection, token) = miyu_ipc::connect(&self.root).await.expect("连得上");
        let (read, mut write) = tokio::io::split(connection);
        let mut read = BufReader::new(read);
        let lines = [
            serde_json::json!({"jsonrpc": "2.0", "id": "plain-1", "method": "hello", "params": {
                "protocol": [1, 1], "head": {"kind": "test", "version": "0"}, "token": token}}),
            serde_json::json!({"jsonrpc": "2.0", "id": "plain-2", "method": "session.create",
                "params": {"cwd": "/work"}}),
        ];
        let mut reply = serde_json::Value::Null;
        for line in lines {
            write
                .write_all(format!("{line}\n").as_bytes())
                .await
                .expect("写得进");
            let mut text = String::new();
            within("回应", read.read_line(&mut text))
                .await
                .expect("读得到");
            reply = serde_json::from_str(&text).expect("是 JSON");
        }
        reply["result"]["session"]
            .as_str()
            .expect("造出来了")
            .to_string()
    }

    /// 管理员的会话，从新到旧。
    pub fn sessions(&self) -> Vec<SessionId> {
        self.root.sessions(&AccountIdOf::admin()).expect("列得出")
    }

    /// 会话 `session` 的日志，只读：会话可能正在写，载入用的 `SessionLog::open` 会截掉正在写的那半行（施工 3-9 下在 macOS 的 CI 上撞到过：会话目录刚建、第一段还没有，它报没有这个会话）。还没写出第一条的当是空的。
    pub fn log(&self, session: &SessionId) -> Vec<Event> {
        read_events(&self.root.session_dir(&AccountIdOf::admin(), session)).unwrap_or_default()
    }

    /// 等到有一个会话开了回合，交回它。最多十秒。
    pub async fn until_a_turn_starts(&self) -> SessionId {
        within("开了回合", async {
            loop {
                for session in self.sessions() {
                    if self
                        .log(&session)
                        .iter()
                        .any(|event| matches!(event.body, Body::TurnStarted(_)))
                    {
                        return session;
                    }
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
        self.running.abort();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// 一个用完就删的临时数据根，建好了骨架。
fn temp_root() -> (PathBuf, DataRoot) {
    let dir = std::env::temp_dir().join(format!("miyu-cli-{}-{}", std::process::id(), next()));
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

/// 套接字放在数据根里，不放系统的运行目录。
pub fn dirs() -> Dirs {
    Dirs {
        runtime_dir: None,
        ..Dirs::current()
    }
}

/// 在真的套接字上连上 `root` 的核心，照 `plan` 说一句。`presses` 是 Ctrl+C。
pub async fn ask_at(root: &DataRoot, plan: &Plan, presses: mpsc::Receiver<()>) -> Asked {
    let (connection, token) = miyu_ipc::connect(root).await.expect("连得上");
    let tape = Tape::default();
    let (mut out, mut err) = (tape.pen(false), tape.pen(true));
    let mut screen = Screen {
        out: &mut out,
        err: &mut err,
        gray: false,
        live: false,
    };
    let code = within("说完", talk(connection, &token, plan, &mut screen, presses)).await;
    Asked {
        code,
        out: tape.text(|err| !err),
        err: tape.text(|err| err),
        screen: tape.text(|_| true),
    }
}

/// 一个没有核心的临时数据根，用完就删：测试自己在它的套接字上当核心。
pub struct Bare {
    dir: PathBuf,
    pub root: DataRoot,
}

impl Bare {
    pub fn new() -> Bare {
        let (dir, root) = temp_root();
        Bare { dir, root }
    }
}

impl Drop for Bare {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// 说完一句看到的：退出码、标准输出、标准错误、整块屏幕（两条通道照先后写在一起，像终端里看到的那样）。
pub struct Asked {
    pub code: u8,
    pub out: String,
    pub err: String,
    pub screen: String,
}

/// 一卷屏幕：标准输出、标准错误照先后记在一起。只看各自的缓冲区看不出两条通道交错的先后。
#[derive(Clone, Default)]
pub struct Tape(Arc<Mutex<Vec<(bool, u8)>>>);

/// 往屏幕上写的一支笔：`err` 的写标准错误。
pub struct Pen {
    tape: Tape,
    err: bool,
}

impl std::io::Write for Pen {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut tape = self.tape.0.lock().expect("没 panic");
        tape.extend(buf.iter().map(|byte| (self.err, *byte)));
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Tape {
    /// 一支笔。
    pub fn pen(&self, err: bool) -> Pen {
        Pen {
            tape: self.clone(),
            err,
        }
    }

    /// 照 `pick` 挑出来的字节，写成字。
    pub fn text(&self, pick: impl Fn(bool) -> bool) -> String {
        let tape = self.0.lock().expect("没 panic");
        let bytes: Vec<u8> = tape
            .iter()
            .filter(|(err, _)| pick(*err))
            .map(|(_, byte)| *byte)
            .collect();
        String::from_utf8(bytes).expect("UTF-8")
    }
}

/// 管理员的账号。
pub struct AccountIdOf;

impl AccountIdOf {
    pub fn admin() -> miyu_kernel::id::AccountId {
        miyu_kernel::id::AccountId::parse("admin").expect("账号合写法")
    }
}

/// 说 `text`：新开一个一次性会话，中文，给人看；给人看的字照出厂的中文那一份。
pub fn plan(text: &str) -> Plan {
    Plan {
        text: text.to_string(),
        target: Target::New,
        format: Format::Text,
        cwd: "/work".to_string(),
        dirs: Vec::new(),
        files: Vec::new(),
        language: Language::Chinese,
        human: Human::load(&resources(), "zh").expect("出厂的字读得出来"),
        home: None,
    }
}

/// 数据根外面的一个临时目录，用完就删：当项目目录，或者当工作区外面。放在 cargo 给集成测试的 `target/tmp`
/// 下面：系统的临时目录整个能读能写，放在里面就造不出「工作区外面」（施工 4-3 下）。
pub struct Outside(pub PathBuf);

impl Outside {
    pub fn new() -> Outside {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "miyu-cli-out-{}-{}",
            std::process::id(),
            next()
        ));
        std::fs::create_dir_all(&dir).expect("建得了目录");
        Outside(dir)
    }

    /// 在里面写一份文件，交回它的路径。
    pub fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, text).expect("写得进");
        path
    }

    /// 这个目录，写成字。
    pub fn text(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Outside {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 等 `future`，最多十秒。
pub async fn within<T>(what: &str, future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .unwrap_or_else(|_| panic!("十秒内没等到{what}"))
}

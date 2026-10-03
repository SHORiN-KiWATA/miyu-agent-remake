//! 几个测试共用的：临时的数据根、拉起真的 `miyu core` 的命令、读核心的运行日志、等核心走。

#![allow(dead_code, reason = "几个测试各用其中一部分")]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use miyu_ipc::{ConnectError, Connection, Lock};
use miyu_store::env::{Env, Platform};
use miyu_store::root::DataRoot;

/// 测试构建出来的主程序。
pub const MIYU: &str = env!("CARGO_BIN_EXE_miyu");

/// 一个永远用不了的主对话的模型（施工 8-11）：驱动还没有，请求当场 `no_model`，一个字节都不往外发。`miyu ask` 说话之前先看
/// `models.chat` 配没配，没配的不造会话；要看「没有可用的模型」那一轮的测试照它写系统配置。
pub const UNUSABLE_MODEL: &str = "[providers.idle]\ndriver = \"openai-responses\"\nbase_url = \"https://idle.invalid\"\nkeys = []\n\n[models]\nchat = \"idle/none\"\n";

/// 一个用完就删的临时数据根，建好了骨架。
pub struct Home {
    pub dir: PathBuf,
    pub root: DataRoot,
}

impl Home {
    pub fn new() -> Home {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("miyu-main-{}-{}-{n}", std::process::id(), stamp()));
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

    /// 系统配置写成 `text`：核心起来之前写。
    pub fn system_config(&self, text: &str) {
        let file = self.root.path().join("system").join("config.toml");
        std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了目录");
        std::fs::write(file, text).expect("写得进");
    }

    /// 拉起核心的命令：`miyu core`，空闲 1 秒就走；数据根是这个临时目录，资源目录是源码树的；不用
    /// `$XDG_RUNTIME_DIR`，没有模型的 key。
    pub fn core(&self) -> Command {
        let mut command = Command::new(MIYU);
        command
            .args(["core", "--idle-seconds", "1"])
            .env("MIYU_HOME", self.root.path())
            .envs(offline(self.root.path()))
            .env("MIYU_RESOURCES", resources())
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("MIYU_LOG");
        command
    }

    /// 核心的运行日志，全文。
    pub fn core_log(&self) -> String {
        std::fs::read_to_string(self.root.state().join("logs").join("core.log")).unwrap_or_default()
    }

    /// 等核心走：连不上了，锁也放开了。最多十秒。
    pub async fn until_stopped(&self) {
        within("核心走了", async {
            loop {
                let gone = matches!(
                    miyu_ipc::connect(&self.root).await,
                    Err(ConnectError::NotRunning)
                );
                if gone && Lock::acquire(&self.root).is_ok() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
    }
}

impl Home {
    /// 停掉头拉起的核心（施工 8-6 起没有 key 也拉起；它空闲十分钟才走）：照运行日志「起来了」那一行的进程号结束它，等它走。
    pub async fn kill_core(&self) {
        let log = self.core_log();
        let pid = log
            .rsplit("pid=")
            .next()
            .filter(|_| log.contains("pid="))
            .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
            .expect("运行日志里有进程号")
            .to_string();
        #[cfg(unix)]
        let killed = Command::new("kill").arg(&pid).status();
        #[cfg(windows)]
        let killed = Command::new("taskkill").args(["/PID", &pid, "/F"]).status();
        assert!(
            killed.is_ok_and(|status| status.success()),
            "结束得了 {pid}"
        );
        self.until_stopped().await;
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

/// 源码树的资源目录。
pub fn resources() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources")
}

/// 等 `future`，最多十秒。
pub async fn within<T>(what: &str, future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .unwrap_or_else(|_| panic!("十秒内没等到{what}"))
}

/// 在连接上握手，交回回应。
pub async fn hello(connection: Connection, token: &str) -> Value {
    let (read, mut write) = tokio::io::split(connection);
    let hello = json!({
        "jsonrpc": "2.0",
        "id": "hello-1",
        "method": "hello",
        "params": {
            "protocol": [1, 1],
            "head": {"kind": "test", "version": "0.0.0"},
            "caps": {"input": false},
            "token": token,
        },
    });
    write
        .write_all(format!("{hello}\n").as_bytes())
        .await
        .expect("写得进");
    let mut line = String::new();
    within("握手的回应", BufReader::new(read).read_line(&mut line))
        .await
        .expect("读得到");
    serde_json::from_str(&line).expect("是 JSON")
}

/// 日志里有几行带着 `words`。
pub fn count(log: &str, words: &str) -> usize {
    log.lines().filter(|line| line.contains(words)).count()
}

/// 起名用的纳秒数：Windows 上进程号复用得快，前一个测试进程留下的、核心开着文件删不掉的目录会撞名（2026-10-01 CI 撞见）。
fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos())
}

/// 在 `dir` 里跑一个会拉起核心的头（施工 8-6 起没有 key 也拉起）：标准输出、标准错误接到这个数据根旁边的文件里，标准输入照 `input`
/// 写完关上。不用管道：Windows 上拉起的核心继承了管道的句柄，等管道关上就要等核心退出（十分钟）。
pub fn run_starting(dir: &Path, mut command: Command, input: &str) -> std::process::Output {
    use std::io::Write;
    use std::process::Stdio;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let n = NEXT.fetch_add(1, Ordering::Relaxed);
    let (out, err) = (
        dir.join(format!("out-{n}.txt")),
        dir.join(format!("err-{n}.txt")),
    );
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(std::fs::File::create(&out).expect("建得了"))
        .stderr(std::fs::File::create(&err).expect("建得了"))
        .spawn()
        .expect("跑得起来");
    let mut stdin = child.stdin.take().expect("有标准输入");
    // 不读标准输入就退出的，写不进去也不要紧。
    let _written = stdin.write_all(input.as_bytes());
    drop(stdin);
    let status = child.wait().expect("等得到");
    std::process::Output {
        status,
        stdout: std::fs::read(&out).expect("读得到"),
        stderr: std::fs::read(&err).expect("读得到"),
    }
}

/// 拉起的核心不去 models.dev 拉目录（施工 8-7，主会话定）：`MIYU_CATALOG_UPDATE=false`；缓存目录指到数据根里的临时一格
/// （Linux 的 `XDG_CACHE_HOME`、Windows 的 `LOCALAPPDATA`），哪个测试漏带了也只写进临时目录。macOS 的缓存目录照家目录，
/// 不改家目录，只靠前一条。
pub fn offline(root: impl AsRef<Path>) -> [(&'static str, PathBuf); 3] {
    let cache = root.as_ref().join("state").join("test-cache");
    [
        ("MIYU_CATALOG_UPDATE", PathBuf::from("false")),
        ("XDG_CACHE_HOME", cache.clone()),
        ("LOCALAPPDATA", cache),
    ]
}

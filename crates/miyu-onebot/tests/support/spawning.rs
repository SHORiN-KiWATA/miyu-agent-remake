//! 真的程序（施工 O-18，`onebot.md` 第一条「守着它的」、「施工时定的」第 28 条）：`miyu-onebot` 的子命令照真的跑；核心照开关
//! 拉起的桥是硬链接在测试程序旁边的 `miyu-onebot`（包的程序只找主程序旁边的，测试里的主程序就是测试程序自己，照核心测扩展
//! 的办法）。不拷：拷的时候开着写的句柄，同一个测试程序里别的测试这时起的子进程会带着它，接着拉起时 Linux 回 `ETXTBSY`。
//! 等真的程序端口听上照状态文件（[`bridge_up`]、[`served_up`]）：挑的空端口被别人先占了的报出来，换一个再来（`ports.rs`）。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;

use miyu_onebot::status_file;
use miyu_store::root::DataRoot;

use super::ports::{Taken, taken};
use super::{resources, within};

/// 测试程序旁边的 `miyu-onebot`：这个测试程序里头一次要的时候链上（[`link_beside`]）。之后留在那里：别的测试可能正跑着它。
pub fn linked() -> &'static Path {
    static LINKED: OnceLock<PathBuf> = OnceLock::new();
    LINKED.get_or_init(|| {
        let exe = std::env::current_exe().expect("找得到测试程序");
        let dir = std::fs::canonicalize(&exe)
            .expect("测试程序在")
            .parent()
            .expect("有上一级")
            .to_path_buf();
        let path = dir.join(format!("miyu-onebot{}", std::env::consts::EXE_SUFFIX));
        link_beside(Path::new(env!("CARGO_BIN_EXE_miyu-onebot")), &path);
        path
    })
}

/// 把 `source` 硬链接到 `path`：已经是同一个文件的不动；不是的（上一次链的是别的构建的），先链到旁边一个只有这一次用的名字，
/// 再改名盖上 `path`。
///
/// 几份测试程序可能同时在链同一个（并着跑同一个测试程序）。原来先删再链：后到的撞 `File exists`，或者把别人刚链好的删掉、
/// 那一刻 `path` 不在（O-23 下撞过）。改名是原子的：`path` 要么是旧的、要么是新的，一直在；谁最后盖上的都是 `source` 那个
/// 文件。所以链、改名的结果都不看，只看最后 `path` 是不是 `source` 那个文件。
///
/// # Panics
///
/// 链、改名完了 `path` 还不是 `source` 那个文件：带上链、改名各自的结果。
pub fn link_beside(source: &Path, path: &Path) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if same(path, source) {
        return;
    }
    let mut name = path.file_name().expect("有文件名").to_os_string();
    name.push(format!(
        ".linking-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let staged = path.with_file_name(name);
    let linked = std::fs::hard_link(source, &staged);
    let renamed = std::fs::rename(&staged, path);
    // 改名成了的旁边那个名字已经没了；`path` 本来就是同一个文件的，改名什么都不做，旁边那个还在。
    if std::fs::remove_file(&staged).is_err() {
        // 不在了：改名成了。
    }
    assert!(
        same(path, source),
        "链不上 {}：链 {linked:?}，改名 {renamed:?}",
        path.display()
    );
}

/// 两个路径是不是同一个文件：大小、改动时刻都对得上（硬链接的两个名字是同一份）。有一个不在的不是。
fn same(one: &Path, other: &Path) -> bool {
    let (Ok(one), Ok(other)) = (std::fs::metadata(one), std::fs::metadata(other)) else {
        return false;
    };
    one.len() == other.len() && one.modified().ok() == other.modified().ok()
}

/// 一个空着的端口：系统挑一个，马上放掉。
pub fn free_port() -> u16 {
    let free = std::net::TcpListener::bind("127.0.0.1:0").expect("挑得到");
    free.local_addr().expect("有地址").port()
}

/// 真的程序 `miyu-onebot`：数据根是 `root`，资源目录是源码树的；系统的语言是英文（握手回中文的，说的话换成中文，才看得出
/// 照握手回的语言说）。不用 `$XDG_RUNTIME_DIR`、`MIYU_LOG`。
pub fn program(root: &DataRoot, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_miyu-onebot"));
    command
        .args(args)
        .env("MIYU_HOME", root.path())
        .env("MIYU_RESOURCES", resources())
        .env("LC_ALL", "en_US.UTF-8")
        .env_remove("XDG_RUNTIME_DIR")
        .env_remove("MIYU_LOG");
    command
}

/// 跑一次 `miyu-onebot <args>` 到完（系统的语言是中文：握手照它定中文），交回它的输出。
pub async fn cli(root: &DataRoot, args: &[&str]) -> Output {
    let mut command = program(root, args);
    command.env("LC_ALL", "zh_CN.UTF-8");
    tokio::task::spawn_blocking(move || command.output())
        .await
        .expect("没崩")
        .expect("跑得了")
}

/// 字节照 UTF-8 读。
pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// 跑 `miyu-onebot status`，直到标准输出合 `wanted`：交回那一次的标准输出。最多等 60 秒。
pub async fn until_status(root: &DataRoot, wanted: impl Fn(&str) -> bool) -> String {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let said = cli(root, &["status"]).await;
        let out = text(&said.stdout);
        if wanted(&out) {
            assert_eq!(said.status.code(), Some(0), "{}", text(&said.stderr));
            return out;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到：{out}{}",
            text(&said.stderr)
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// 核心的 `extension.status` 里桥的那一个。
pub async fn extension(root: &DataRoot) -> Value {
    let mut core = miyu_webserve::open::Core::connect_running(root, "test")
        .await
        .expect("连得上核心");
    let listed = core
        .call("s", "extension.status", serde_json::json!({}))
        .await
        .expect("答得了");
    listed["extensions"]
        .as_array()
        .expect("是数组")
        .iter()
        .find(|one| one["package"] == "onebot")
        .cloned()
        .unwrap_or_else(|| panic!("没有 onebot：{listed}"))
}

/// 等到桥的那一个合 `wanted`：最多 60 秒。
pub async fn until_extension(root: &DataRoot, wanted: impl Fn(&Value) -> bool) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let one = extension(root).await;
        if wanted(&one) {
            return one;
        }
        assert!(tokio::time::Instant::now() < deadline, "等不到：{one}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 等到 `127.0.0.1:port` 连得上（`open` 是真）或者连不上（假）：最多 60 秒。
pub async fn until_port(port: u16, open: bool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .is_ok()
        != open
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "端口 {port} 等不到 {open}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 等核心拉起的桥端口听上：`extension.status` 说在跑、状态文件的进程号是它的（端口绑上了才写）。`before` 是上一个桥的
/// 进程号的，等的是换了进程号的那一个（被杀、重启以后核心拉起的新的）。交回它的进程号。桥停下了、标准错误说的是 `listen`
/// 被占了：[`Taken`]（`ports.rs`）；别的原因停下的当失败。
pub async fn bridge_up(root: &DataRoot, listen: u16, before: Option<u64>) -> Result<u64, Taken> {
    let one = until_extension(root, |one| {
        let pid = one["pid"].as_u64();
        one["state"] == "stopped"
            || (one["state"] == "running"
                && pid != before
                && status_file::read(root).is_some_and(|file| file["pid"].as_u64() == pid))
    })
    .await;
    if one["state"] == "stopped" {
        assert!(
            taken(one["stderr"].as_str().unwrap_or_default(), listen),
            "桥停下了：{one}"
        );
        return Err(Taken);
    }
    Ok(one["pid"].as_u64().expect("在跑的有进程号"))
}

/// 杀掉进程 `pid`，不让它收拾：Unix 上 `kill -9`；Windows 上 `Stop-Process -Force`（退出码是 -1，不是 `taskkill /F` 的 1：
/// 退出码 1 核心当配置错、不再拉起）。
pub fn kill(pid: u64) {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("powershell");
        command.args([
            "-NoProfile",
            "-Command",
            &format!("Stop-Process -Id {pid} -Force"),
        ]);
        command
    } else {
        let mut command = Command::new("kill");
        command.args(["-9", &pid.to_string()]);
        command
    };
    let status = command.status().expect("跑得了");
    assert!(status.success(), "杀得掉 {pid}：{status}");
}

/// 测试当核心跑着的真的 `serve`：核心那一头是它的标准输入输出（施工 O-18）。
pub struct Served {
    /// 那个进程。放下就杀掉。
    pub child: tokio::process::Child,
    /// 它发的握手。
    pub hello: Value,
    /// 它的标准输入：放下就是请它退出。
    pub stdin: Option<tokio::process::ChildStdin>,
    /// 它的标准输出上的每一行（握手也算），读到头才交回。
    pub stdout: tokio::task::JoinHandle<Vec<String>>,
    /// 到这一刻为止标准输出上的每一行（握手以后的，施工 O-28 上：等它回核心发去的请求，不用等它退出）。
    pub seen: Arc<Mutex<Vec<String>>>,
}

impl Served {
    /// 照核心的样子推一次配置的变化（`extension.config`，施工 O-20）：`keys` 是 `{键: 新值或 null}`。
    pub async fn config(&mut self, keys: Value) {
        let pushed = serde_json::json!({"jsonrpc": "2.0", "method": "extension.config", "params": {"keys": keys}});
        self.send(&pushed).await;
    }

    /// 照核心的样子往它的标准输入写一行 `message`（推送、核心发去的请求，施工 O-28 上）。
    pub async fn send(&mut self, message: &Value) {
        use tokio::io::AsyncWriteExt;

        let stdin = self.stdin.as_mut().expect("标准输入还开着");
        stdin
            .write_all(format!("{message}\n").as_bytes())
            .await
            .expect("写得进");
        stdin.flush().await.expect("写得出");
    }

    /// 等到它在标准输出上回了编号是 `id` 的那一条（施工 O-28 上），交回它。最多等十秒。
    pub async fn answer(&self, id: &str) -> Value {
        within("桥回核心", async {
            loop {
                let found = self
                    .seen
                    .lock()
                    .expect("没 panic")
                    .iter()
                    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                    .find(|one| one["id"] == id && one.get("method").is_none());
                if let Some(found) = found {
                    return found;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
    }
}

/// 照 [`program`] 跑 `serve`，测试当核心：读它的握手，回 `language: zh`，`config` 照核心拉起扩展时交的样子是 `config`
/// （施工 O-20，`extensions.md`「配置」），之后它在标准输出上写的都收着。
pub async fn served(root: &DataRoot, config: Value) -> Served {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let mut command = tokio::process::Command::from(program(root, &["serve"]));
    command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().expect("起得来");
    let mut stdin = child.stdin.take().expect("接了管道");
    let mut lines = BufReader::new(child.stdout.take().expect("接了管道")).lines();
    let first = super::within("桥发握手", lines.next_line())
        .await
        .expect("读得了")
        .expect("桥发了握手");
    let hello: Value = serde_json::from_str(&first).expect("握手是一行 JSON");
    let reply = serde_json::json!({
        "jsonrpc": "2.0",
        "id": hello["id"],
        "result": {"protocol": 1, "account": "admin", "language": "zh", "config": config},
    });
    stdin
        .write_all(format!("{reply}\n").as_bytes())
        .await
        .expect("写得进");
    stdin.flush().await.expect("写得出");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let seeing = Arc::clone(&seen);
    let stdout = tokio::spawn(async move {
        let mut all = vec![first];
        while let Ok(Some(line)) = lines.next_line().await {
            seeing.lock().expect("没 panic").push(line.clone());
            all.push(line);
        }
        all
    });
    Served {
        child,
        hello,
        stdin: Some(stdin),
        stdout,
        seen,
    }
}

/// 照 [`served`] 跑 `serve`（握手交的端口是 `listen`，没有令牌），等它端口听上：状态文件的进程号是它的。它退了、标准错误
/// 说的是这个端口被占了：[`Taken`]（`ports.rs`）；别的原因退出的当失败。
pub async fn served_up(root: &DataRoot, listen: u16) -> Result<Served, Taken> {
    served_with(root, serde_json::json!({"onebot.listen": listen}), listen).await
}

/// 同 [`served_up`]，握手交的配置是 `config`（里面的 `onebot.listen` 是 `listen`）。
pub async fn served_with(root: &DataRoot, config: Value, listen: u16) -> Result<Served, Taken> {
    let mut served = served(root, config).await;
    let pid = served.child.id().map(u64::from);
    let up = within("桥端口听上", async {
        loop {
            if status_file::read(root).is_some_and(|file| file["pid"].as_u64() == pid) {
                return true;
            }
            if served.child.try_wait().expect("看得到").is_some() {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await;
    if up {
        return Ok(served);
    }
    let exited = served.child.wait_with_output().await.expect("等得到");
    let said = text(&exited.stderr);
    assert!(taken(&said, listen), "桥退了：{}，{said}", exited.status);
    Err(Taken)
}

/// 系统配置：端口照写，令牌是 `onebot.token` 引用密钥 `onebot`（密钥文件由 `Home::spawning` 写），说中文。终端管理员对应表由
/// `Home` 写在前面。
pub fn ports_config(listen: u16) -> String {
    ports_config_with(listen, "")
}

/// 同 [`ports_config`]，`[onebot]` 里接着写 `onebot`（一行一项，例如白名单成员，施工 O-23）。
pub fn ports_config_with(listen: u16, onebot: &str) -> String {
    format!(
        "\n[onebot]\nlisten = {listen}\ntoken = {{ secret = \"onebot\" }}\n{onebot}\n[ui]\nlanguage = \"zh\"\n"
    )
}

//! 真的程序（施工 O-18，`onebot.md` 第一条「守着它的」、「施工时定的」第 28 条）：`miyu-onebot` 的子命令照真的跑；核心照开关
//! 拉起的桥是硬链接在测试程序旁边的 `miyu-onebot`（包的程序只找主程序旁边的，测试里的主程序就是测试程序自己，照核心测扩展
//! 的办法）。不拷：拷的时候开着写的句柄，同一个测试程序里别的测试这时起的子进程会带着它，接着拉起时 Linux 回 `ETXTBSY`。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::Duration;

use serde_json::Value;

use miyu_store::root::DataRoot;

use super::resources;

/// 测试程序旁边的 `miyu-onebot`：这个测试程序里头一次要的时候链上。上一次链的是别的构建的（大小、改动时刻对不上）先删掉
/// 再链。之后留在那里：别的测试可能正跑着它。
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
        let source = Path::new(env!("CARGO_BIN_EXE_miyu-onebot"));
        let same = |one: &Path, other: &Path| {
            let (Ok(one), Ok(other)) = (std::fs::metadata(one), std::fs::metadata(other)) else {
                return false;
            };
            one.len() == other.len() && one.modified().ok() == other.modified().ok()
        };
        if !same(&path, source) {
            if let Err(error) = std::fs::remove_file(&path)
                && error.kind() != std::io::ErrorKind::NotFound
            {
                panic!("删不掉上一次链的 {}：{error}", path.display());
            }
            std::fs::hard_link(source, &path).expect("链得上");
        }
        path
    })
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
}

/// 照 [`program`] 跑 `serve`，测试当核心：读它的握手，回 `language: zh`，之后它在标准输出上写的都收着。
pub async fn served(root: &DataRoot) -> Served {
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
        "result": {"protocol": 1, "account": "admin", "language": "zh"},
    });
    stdin
        .write_all(format!("{reply}\n").as_bytes())
        .await
        .expect("写得进");
    stdin.flush().await.expect("写得出");
    let stdout = tokio::spawn(async move {
        let mut all = vec![first];
        while let Ok(Some(line)) = lines.next_line().await {
            all.push(line);
        }
        all
    });
    Served {
        child,
        hello,
        stdin: Some(stdin),
        stdout,
    }
}

/// 系统配置：两个端口照写，令牌是 `onebot.token` 引用密钥 `onebot`（密钥文件另写），说中文。主人对应表由 `Home` 写在前面。
pub fn ports_config(listen: u16, web: u16) -> String {
    format!(
        "\n[onebot]\nlisten = {listen}\nweb = {web}\ntoken = {{ secret = \"onebot\" }}\n\n[ui]\nlanguage = \"zh\"\n"
    )
}

/// 往数据根 `root` 的密钥文件里存 `onebot = <value>`。
pub fn store_token(root: &DataRoot, value: &str) {
    let system = root.path().join("system");
    std::fs::create_dir_all(&system).expect("建得了");
    std::fs::write(
        system.join("secrets.toml"),
        format!("onebot = \"{value}\"\n"),
    )
    .expect("写得进");
}

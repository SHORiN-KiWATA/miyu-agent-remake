//! `miyu-try`：试玩台（`docs/construction/3-5-试玩台（补）.md`）。
//!
//! ```text
//! cargo run -p miyu-try -- [--env <key 文件>] [--model <模型>] [--system <文件>]
//! ```
//!
//! key 文件默认 `~/.config/miyu-try/deepseek.env`。数据写到 `MIYU_HOME`；没设的，每次开一个新的
//! 临时目录，开头打印路径。标准输入一行一句；Ctrl+C 打断这一轮，空闲时 Ctrl+C 或者输入完了就退出。

use std::io::{BufRead, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use miyu_http::{Endpoint, Proxy};
use miyu_kernel::id::{ModelName, ProviderId};
use miyu_try::{Bench, KeyFile, SYSTEM, Said, Show};
use tokio::sync::mpsc;

/// 多久没收到新的字节就算断了。思考模型想得久也是一段一段地出，两分钟一个字都没有就是断了。
const IDLE: Duration = Duration::from_secs(120);

const USAGE: &str = "用法：miyu-try [--env <key 文件>] [--model <模型>] [--system <文件>]";

/// 命令行上给的。
struct Args {
    env: Option<PathBuf>,
    model: Option<String>,
    system: Option<PathBuf>,
}

impl Args {
    /// 读命令行，不认识的、少了值的，交回怎么用。
    fn parse(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
        let mut parsed = Args {
            env: None,
            model: None,
            system: None,
        };
        while let Some(arg) = args.next() {
            let mut value = || {
                args.next()
                    .ok_or_else(|| format!("{arg} 后面少了值\n{USAGE}"))
            };
            match arg.as_str() {
                "--env" => parsed.env = Some(PathBuf::from(value()?)),
                "--model" => parsed.model = Some(value()?),
                "--system" => parsed.system = Some(PathBuf::from(value()?)),
                "-h" | "--help" => return Err(USAGE.to_string()),
                _ => return Err(format!("不认识 {arg}\n{USAGE}")),
            }
        }
        Ok(parsed)
    }
}

fn main() -> ExitCode {
    match start() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// 读好配置，接上终端，跑到退出。
fn start() -> Result<(), String> {
    let args = Args::parse(std::env::args().skip(1))?;
    let path = args
        .env
        .or_else(KeyFile::default_path)
        .ok_or("找不到家目录，用 --env 指明 key 文件")?;
    let key_file = KeyFile::read(&path).map_err(|error| error.to_string())?;
    let model = args.model.unwrap_or(key_file.model);
    let system = match &args.system {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|error| format!("读不到 {}：{error}", path.display()))?,
        None => SYSTEM.to_string(),
    };
    let bench = Bench {
        endpoint: Endpoint::new(key_file.base_url, key_file.key),
        provider: ProviderId::parse("deepseek").map_err(|error| error.to_string())?,
        model: ModelName::parse(&model).map_err(|error| format!("模型名 {model}：{error}"))?,
        system,
        root: data_root()?,
        proxy: Proxy::FromEnvironment,
        idle: IDLE,
    };
    let interactive = std::io::stdin().is_terminal();
    let color = std::io::stdout().is_terminal();
    let (tell, said) = mpsc::unbounded_channel();
    read_lines(tell.clone());
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    let dir = runtime.block_on(async move {
        watch_ctrl_c(tell);
        miyu_try::run(
            bench,
            said,
            Show::new(std::io::stdout(), color),
            interactive,
        )
        .await
    });
    let dir = dir.map_err(|error| error.to_string())?;
    println!("（日志在 {}）", dir.display());
    Ok(())
}

/// 数据写到哪：设了 `MIYU_HOME` 的照它；没设的，临时目录下新开一个。不落到家目录的 `.miyu`：那里
/// 现在是旧版在用的数据。
fn data_root() -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("MIYU_HOME").filter(|home| !home.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    Ok(std::env::temp_dir().join(format!("miyu-try-{stamp}")))
}

/// 标准输入一行一句，读完了说一声。在一个线程里读：读标准输入会一直等着。
fn read_lines(tell: mpsc::UnboundedSender<Said>) {
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tell.send(Said::Line(line)).is_err() {
                return;
            }
        }
        #[expect(
            clippy::let_underscore_must_use,
            reason = "试玩台已经退出了，没人要听，不要紧"
        )]
        let _ = tell.send(Said::End);
    });
}

/// Ctrl+C 一次说一声打断。
fn watch_ctrl_c(tell: mpsc::UnboundedSender<Said>) {
    tokio::spawn(async move {
        while tokio::signal::ctrl_c().await.is_ok() {
            if tell.send(Said::Interrupt).is_err() {
                return;
            }
        }
    });
}

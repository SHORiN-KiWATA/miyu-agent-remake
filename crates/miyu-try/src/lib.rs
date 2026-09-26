//! 试玩台（`docs/construction/3-5-试玩台（补）.md`）：内核直接接真模型，跳过核心进程和协议端点，
//! 在终端里一行一行地说。只能聊天，工具一件都没有。
//!
//! 它是工具 crate，不属于哪一层，谁也不许依赖它（`docs/designs/01-架构.md` 第九节第 5 条）：内核、
//! 组装、驱动、HTTP、存储在这里一股脑接在一个进程里，以后是核心（3-7、3-8）和头（3-9）分开做的事。
//!
//! - [`KeyFile`]：key 文件，仓库外、只有本人能读；
//! - [`run()`]：开一个会话，照人说的一行一行跑，跑完交回会话的目录；
//! - [`Show`]：往终端写。

mod call;
mod config;
mod policy;
mod run;
mod show;

pub use call::{Cut, Within};
pub use config::{KeyFile, KeyFileError};
pub use policy::SYSTEM;
pub use run::Said;
pub use show::Show;

use std::collections::VecDeque;
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use miyu_drivers::{Call, Inputs, OpenAiChat};
use miyu_http::{Endpoint, Proxy};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{ModelName, ProviderId};
use miyu_kernel::origin::Model;
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_store::env::Env;
use miyu_store::root::{DataRoot, PrepareError, RootError};
use tokio::sync::mpsc;

use call::Caller;
use run::Executor;

/// 试玩台要的：发给谁、用哪个模型、system、数据写到哪。
#[derive(Debug)]
pub struct Bench {
    /// 地址和 key。
    pub endpoint: Endpoint,
    /// 供应商的编号，记进 `model.called`。
    pub provider: ProviderId,
    /// 模型。
    pub model: ModelName,
    /// system：占位的 [`SYSTEM`]，或者 `--system` 读进来的。
    pub system: String,
    /// 数据根，绝对路径。
    pub root: PathBuf,
    /// 走不走代理。
    pub proxy: Proxy,
    /// 多久没收到新的字节就算断了。
    pub idle: Duration,
}

/// 试玩台跑不下去了。
#[derive(Debug)]
pub enum BenchError {
    /// 数据根找不到。
    Root(RootError),
    /// 数据根建不了骨架，或者认不出是 Miyu 的数据根。
    Prepare(PrepareError),
    /// HTTP 客户端造不出来。
    Client(reqwest::Error),
    /// 取不到随机数，会话编号造不出来。
    Random(getrandom::Error),
    /// 写日志、写终端出错。
    Io(io::Error),
    /// 编号的写法不对：试玩台自己的 bug。
    Name(String),
}

impl BenchError {
    /// 编号的写法不对。
    fn name(error: impl fmt::Display) -> BenchError {
        BenchError::Name(error.to_string())
    }
}

impl fmt::Display for BenchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BenchError::Root(error) => write!(f, "{error}"),
            BenchError::Prepare(error) => write!(f, "{error}"),
            BenchError::Client(error) => write!(f, "HTTP 客户端造不出来：{error}"),
            BenchError::Random(error) => write!(f, "取不到随机数：{error}"),
            BenchError::Io(error) => write!(f, "写不进去：{error}"),
            BenchError::Name(error) => write!(f, "试玩台自己的编号写错了：{error}"),
        }
    }
}

impl std::error::Error for BenchError {}

impl From<io::Error> for BenchError {
    fn from(error: io::Error) -> BenchError {
        BenchError::Io(error)
    }
}

impl From<RootError> for BenchError {
    fn from(error: RootError) -> BenchError {
        BenchError::Root(error)
    }
}

impl From<PrepareError> for BenchError {
    fn from(error: PrepareError) -> BenchError {
        BenchError::Prepare(error)
    }
}

impl From<reqwest::Error> for BenchError {
    fn from(error: reqwest::Error) -> BenchError {
        BenchError::Client(error)
    }
}

/// 开一个会话，照 `said` 一行一行跑：空闲时读一行发出去，一轮走完了再读下一行；一轮在走的时候来了
/// Ctrl+C 就打断它，空闲时来了就退出；输入完了，这一轮走完了退出。`interactive` 为真的是有人在
/// 终端前：空闲时写提示；不是的（管道、测试），把读到的那一行照写一遍，记录读得通。
///
/// 交回会话的目录，日志在里面。
///
/// # Errors
///
/// 数据根找不到、建不了、认不出；HTTP 客户端造不出来；写日志、写终端出错。
pub async fn run<W: Write>(
    bench: Bench,
    mut said: mpsc::UnboundedReceiver<Said>,
    show: Show<W>,
    interactive: bool,
) -> Result<PathBuf, BenchError> {
    let env = Env {
        miyu_home: Some(bench.root.clone().into_os_string()),
        ..Env::current()
    };
    let root = DataRoot::locate(&env)?;
    root.prepare()?;
    let caller = Caller {
        client: miyu_http::client(bench.proxy)?,
        endpoint: Arc::new(bench.endpoint),
        driver: Arc::new(OpenAiChat::new(policy::compat(), policy::driver_texts())),
        call: Call {
            model: bench.model.clone(),
            max_output: None,
            inputs: Inputs::default(),
        },
        model: Model {
            endpoint: bench.provider,
            model: bench.model.clone(),
        },
        idle: bench.idle,
    };
    let (inputs, mut replies) = mpsc::unbounded_channel();
    let sessions = |account: &_, session: &_| root.session_dir(account, session);
    let mut executor =
        Executor::open(sessions, &bench.system, environment(), caller, show, inputs)?;
    let dir = executor.dir().clone();
    executor.show().line(&format!(
        "试玩台 · {} · 日志在 {}",
        bench.model,
        dir.display()
    ))?;
    executor.show().line(
        "说话回车；Ctrl+C 打断这一轮，空闲时 Ctrl+C 或者 Ctrl+D 退出；/cut 下一次在思考里掐断，/cut text 在回复里掐断；/quit 退出。",
    )?;
    let mut held = VecDeque::new();
    let mut open = true;
    let mut ending = false;
    loop {
        if executor.idle() {
            let next = match held.pop_front() {
                Some(line) => Said::Line(line),
                None if ending || !open => break,
                None => {
                    if interactive {
                        executor.show().prompt()?;
                    }
                    match said.recv().await {
                        Some(next) => next,
                        None => break,
                    }
                }
            };
            match next {
                Said::Line(line) if line.trim() == "/quit" => break,
                Said::Line(line) if line.trim().is_empty() => {}
                Said::Line(line) => {
                    if !interactive {
                        executor.show().line(&format!("你> {line}"))?;
                    }
                    executor.say(line.trim())?;
                }
                Said::Interrupt | Said::End => break,
            }
            continue;
        }
        tokio::select! {
            Some(input) = replies.recv() => executor.feed(input)?,
            next = said.recv(), if open => match next {
                Some(Said::Line(line)) => held.push_back(line),
                Some(Said::Interrupt) => executor.interrupt()?,
                Some(Said::End) | None => {
                    open = false;
                    ending = true;
                }
            },
            else => break,
        }
    }
    Ok(dir)
}

/// 现在，取自系统的时钟。
///
/// # Panics
///
/// 实际不会 panic：现在的时刻在能写的范围里。
pub(crate) fn now() -> Timestamp {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
        });
    Timestamp::from_unix_millis(ms).expect("现在的时刻在能写的范围里")
}

/// 一个新的会话编号：UUIDv7 的写法，前 48 位是现在的毫秒，其余是随机数。
pub(crate) fn session_id() -> Result<String, BenchError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(BenchError::Random)?;
    let ms = u64::try_from(now().unix_millis()).unwrap_or(0);
    bytes[..6].copy_from_slice(&ms.to_be_bytes()[2..]);
    bytes[6] = (bytes[6] & 0x0f) | 0x70;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}

/// 会话所在的环境：系统的时区，现在的工作目录，家目录写成 `~`。
fn environment() -> Environment {
    let seconds = jiff::Zoned::now().offset().seconds();
    let offset = UtcOffset::from_minutes(seconds / 60)
        .or_else(|| UtcOffset::from_minutes(0))
        .expect("零时区在范围里");
    let cwd = std::env::current_dir()
        .map(|cwd| tilde(&cwd, std::env::home_dir().as_deref()))
        .unwrap_or_else(|_| "~".to_string());
    Environment { offset, cwd }
}

/// 家目录底下的路径写成 `~` 开头：省字，也不把用户名发出去。
fn tilde(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ids_have_the_uuid_v7_shape() {
        let id = session_id().unwrap();
        assert!(miyu_kernel::id::SessionId::parse(&id).is_ok(), "{id}");
        assert_eq!(&id[14..15], "7");
        assert_ne!(id, session_id().unwrap());
    }

    #[test]
    fn paths_under_home_start_with_a_tilde() {
        let home = Path::new("/home/me");
        assert_eq!(
            tilde(Path::new("/home/me/src/miyu"), Some(home)),
            "~/src/miyu"
        );
        assert_eq!(tilde(Path::new("/home/me"), Some(home)), "~");
        assert_eq!(tilde(Path::new("/srv/x"), Some(home)), "/srv/x");
        assert_eq!(tilde(Path::new("/srv/x"), None), "/srv/x");
    }
}

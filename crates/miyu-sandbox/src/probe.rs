//! 探测（`docs/blueprint/sandbox.md`「怎么走」第 2 条）：跑一次助手的 `probe`，读它说这台机器能收紧到什么程度。
//! 核心起来时探一次，只记日志。
//!
//! 等它有上限：读输出、等它退出加起来不过 `timeout`，到时杀掉它。读输出在另一个线程里：它放出去的东西拿着管道不放
//! 的，也不会一直等下去。

use std::fmt;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// 这份说法的版本。改了写法就加一：核心只认它认得的。
pub const VERSION: u32 = 1;

/// 等它退出时多久看一次。
const POLL: Duration = Duration::from_millis(10);

/// 助手说的：这台机器能收紧到什么程度。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Probe {
    /// 这份说法的版本：[`VERSION`]。
    pub version: u32,
    /// 哪个平台。
    pub platform: Platform,
    /// 能用上的收紧手段。施工 5-1 是空的，5-2 起各平台往里加。
    pub mechanisms: Vec<String>,
}

/// 助手是为哪个平台编的。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    /// Linux。
    Linux,
    /// macOS。
    Macos,
    /// Windows。
    Windows,
    /// 别的。
    Other,
}

impl Platform {
    /// 编的是哪个平台。
    pub const fn current() -> Platform {
        if cfg!(target_os = "linux") {
            Platform::Linux
        } else if cfg!(target_os = "macos") {
            Platform::Macos
        } else if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Other
        }
    }

    /// 写进 JSON、日志里的名字。
    pub const fn name(self) -> &'static str {
        match self {
            Platform::Linux => "linux",
            Platform::Macos => "macos",
            Platform::Windows => "windows",
            Platform::Other => "other",
        }
    }
}

impl Probe {
    /// 这台机器上的：这一版、编的是哪个平台，能用上的手段是 `mechanisms`（助手的各平台文件报的）。
    pub fn new(mechanisms: Vec<String>) -> Probe {
        Probe {
            version: VERSION,
            platform: Platform::current(),
            mechanisms,
        }
    }

    /// 日志里的手段：逗号连起来，空的写 `none`。
    pub fn mechanisms_text(&self) -> String {
        if self.mechanisms.is_empty() {
            "none".to_string()
        } else {
            self.mechanisms.join(",")
        }
    }
}

/// 探不成：日志里的 `reason`。
#[derive(Debug)]
pub enum ProbeError {
    /// 起不来，或者等不了、读不了它的输出。
    Start(io::Error),
    /// 到时了还没说完、没退出，杀掉了。
    TimedOut,
    /// 退出码不是 0。
    Exited(ExitStatus),
    /// 说的读不懂：不是那一行 JSON，或者版本不认得。
    Unreadable(String),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProbeError::Start(error) => write!(f, "cannot run helper: {error}"),
            ProbeError::TimedOut => f.write_str("helper timed out"),
            ProbeError::Exited(status) => write!(f, "helper failed: {status}"),
            ProbeError::Unreadable(why) => write!(f, "helper output not understood: {why}"),
        }
    }
}

impl std::error::Error for ProbeError {}

/// 跑一次 `<helper> probe`，最多等 `timeout`。
///
/// # Errors
///
/// 起不来、到时了、退出码不是 0、说的读不懂（[`ProbeError`]）。
pub fn probe(helper: &Path, timeout: Duration) -> Result<Probe, ProbeError> {
    let deadline = Instant::now() + timeout;
    let mut child = Command::new(helper)
        .arg("probe")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(ProbeError::Start)?;
    let read = reader(&mut child);
    let bytes = match read.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => {
            stop(child);
            return Err(ProbeError::Start(error));
        }
        Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
            stop(child);
            return Err(ProbeError::TimedOut);
        }
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                stop(child);
                return Err(ProbeError::TimedOut);
            }
            Ok(None) => thread::sleep(POLL),
            Err(error) => {
                stop(child);
                return Err(ProbeError::Start(error));
            }
        }
    };
    if !status.success() {
        return Err(ProbeError::Exited(status));
    }
    let probe = serde_json::from_slice::<Probe>(&bytes)
        .map_err(|error| ProbeError::Unreadable(error.to_string()))?;
    if probe.version != VERSION {
        return Err(ProbeError::Unreadable(format!("version {}", probe.version)));
    }
    Ok(probe)
}

/// 在另一个线程里把标准输出读完：读完了（它关了管道）交回来。
fn reader(child: &mut Child) -> mpsc::Receiver<io::Result<Vec<u8>>> {
    let (send, receive) = mpsc::channel();
    let stdout = child.stdout.take();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = match stdout {
            Some(mut stdout) => stdout.read_to_end(&mut bytes).map(|_| bytes),
            None => Err(io::Error::other("helper stdout not captured")),
        };
        #[expect(
            clippy::let_underscore_must_use,
            reason = "等的那一头到时就不等了，没人收也不要紧"
        )]
        let _ = send.send(read);
    });
    receive
}

/// 不等了：杀掉它，收走。已经退出了的，杀不杀都一样。
fn stop(mut child: Child) {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "已经退出了的杀不了，不要紧；接着收走它"
    )]
    let _ = child.kill();
    #[expect(
        clippy::let_underscore_must_use,
        reason = "收走只为不留僵尸进程，收不了也没有别的办法"
    )]
    let _ = child.wait();
}

#[cfg(test)]
mod tests;

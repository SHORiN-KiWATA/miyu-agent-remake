//! `miyu onebot start`、`stop`、`restart`、`status`（`onebot.md` 第一条「对外的样子」，施工 O-18；18 第三节 Q17）：开关存在
//! 核心那边（`extensions.md`），这里只调核心的方法、照回应说。
//!
//! 1. 照终端的样子连核心：出示本机令牌，没在跑就拉起（「施工时定的」第 24 条：开关在核心那边，核心不在开不了也关不了），
//!    用 `miyu_client::open::Core`（和网页软件同一个，「施工时定的」第 166 条）；握手以后照核心回的语言说。连不上：
//!    `failure/core`，退出码 1。
//! 2. `start`、`stop`、`restart` 调 `extension.enable`、`disable`、`restart`，`status` 调 `extension.status` 取 [`PACKAGE`]
//!    那一个。核心拒绝的照核心的原话说（它已经照连接的语言说了），退出码 1。核心那边没有这个包：[`Report::Missing`]，退出码 1。
//! 3. 照那一个说（[`describe`]）：`start`、`restart` 先说一句做了什么，再说它这时的样子；`stop` 只说关了；`status` 说它的样子，
//!    在跑的、状态文件（`crate::status_file`）的进程号对得上的，再说 NapCat、NapCat 那边的地址（施工 O-28 下）。`start`、
//!    `status` 末尾一律接一句设置和状态在网页里、用 `miyu onebot web` 打开（[`Report::Page`]，施工 O-28 补，「施工时定的」
//!    第 174 条）：关着、停下了的也说，人要去那里设令牌、改端口；`restart` 不说。
//!
//! 说的印在标准输出上，出错的在标准错误上。

use std::io::Write;

use serde_json::{Value, json};

use miyu_client::open::Core;
use miyu_store::root::DataRoot;

/// 拉起核心的命令（`start` 这几样连核心时核心没在跑就照它拉起），和网页软件同一个写法。
pub use miyu_client::CoreCommand;

use crate::serve::Failure;
use crate::texts::Texts;
use crate::{PACKAGE, status_file};

/// 没办成的退出码。
const FAILED: u8 = 1;

/// 要做的那一件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    /// 打开开关，核心拉起桥。
    Start,
    /// 关开关，核心请桥退出。
    Stop,
    /// 核心请桥退出、重新拉起。
    Restart,
    /// 说桥这时的样子。
    Status,
}

/// 说给人听的一句（「给人看的字」`control/`、`status/`）：怎么说照 `texts`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// `start` 成了。
    Started,
    /// `stop` 成了。
    Stopped,
    /// `restart` 成了。
    Restarted,
    /// 关着。
    Off,
    /// 拉起了、还没握手。
    Starting,
    /// 在跑：进程号。
    Running(u64),
    /// 退避中：几秒后再拉起（进成整秒）、连续失败了几次。
    Waiting {
        /// 几秒后再拉起。
        seconds: u64,
        /// 连续失败了几次。
        failures: u64,
    },
    /// 停下了，核心不再拉起：为什么。
    Halted(Halt),
    /// 接着几行是它的标准错误的最后几行。
    Stderr,
    /// 不认识的状态（照说不会）：照原样。
    Other(String),
    /// 核心那边没有这个包：清单不在、写错了。
    Missing,
    /// NapCat 连着、问到了是哪个实现：实现、版本、机器人的号。
    Napcat {
        /// 实现的名字。
        implementation: String,
        /// 实现的版本。
        version: String,
        /// 机器人的号。
        bot: String,
    },
    /// NapCat 连着、还没问到是哪个实现：机器人的号。
    NapcatBot(String),
    /// NapCat 没连着。
    NoNapcat,
    /// NapCat 那边的地址：实际听的端口（施工 O-28 下：原来连着网页的地址一起说）。
    Listen(u64),
    /// 设置和状态在网页里、用 `miyu onebot web` 打开（施工 O-28 下；O-28 补改了说法）：`start`、`status` 说完跟上。
    Page,
}

/// 停下的原因（`extensions.md`「对外的样子」的 `reason`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Halt {
    /// `config_error`：退出码 1，配置错、端口被占。
    ConfigError,
    /// `failed_repeatedly`：连续失败了几次。
    FailedRepeatedly(u64),
    /// `not_installed`：程序不在 `miyu` 旁边。
    NotInstalled,
    /// `cannot_start`：起不来。
    CannotStart,
    /// `protocol_mismatch`：清单的协议版本对不上。
    ProtocolMismatch,
    /// 不认识的：照原样印代码。
    Other(String),
}

/// 印出来的一行：说的一句，或者标准错误里原样的一行（缩进两格印）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// 照 `texts` 说的一句。
    Say(Report),
    /// 标准错误里的一行，原样。
    Tail(String),
}

/// 照 `which` 走一遍，交回退出码。核心没在跑时照 `core` 拉起；说的话照 `texts`，握手以后换成核心回的语言。
pub async fn control(
    root: &DataRoot,
    which: Control,
    core: &CoreCommand,
    texts: &mut Texts,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut core = match Core::connect(root, core, "onebot").await {
        Ok(core) => core,
        Err(reason) => {
            say(err, &texts.failure(&Failure::Core(reason)));
            return FAILED;
        }
    };
    if let Some(language) = core.language()
        && let Err(error) = texts.speak(language)
    {
        // 那种语言的字读不懂：照实说一行，接着照原来的说。
        say(err, &format!("{}: {error}", crate::PROGRAM));
    }
    let package = json!({"package": PACKAGE});
    let (method, params, done) = match which {
        Control::Start => ("extension.enable", package, Some(Report::Started)),
        Control::Stop => ("extension.disable", package, Some(Report::Stopped)),
        Control::Restart => ("extension.restart", package, Some(Report::Restarted)),
        Control::Status => ("extension.status", json!({}), None),
    };
    let answered = match core.call("onebot", method, params).await {
        Ok(answered) => answered,
        Err(said) => {
            say(err, &said);
            return FAILED;
        }
    };
    let entry = match which {
        Control::Status => answered["extensions"]
            .as_array()
            .and_then(|all| all.iter().find(|one| one["package"] == PACKAGE))
            .cloned(),
        _ => Some(answered),
    };
    let Some(entry) = entry else {
        say(err, &texts.report(&Report::Missing));
        return FAILED;
    };
    let mut lines: Vec<Line> = done.into_iter().map(Line::Say).collect();
    if which != Control::Stop {
        lines.extend(describe(&entry, status_file::read(root).as_ref()));
    }
    if matches!(which, Control::Start | Control::Status) {
        lines.push(Line::Say(Report::Page));
    }
    for line in lines {
        match line {
            Line::Say(report) => say(out, &texts.report(&report)),
            Line::Tail(tail) => say(out, &format!("  {tail}")),
        }
    }
    0
}

/// `extension.status` 里桥的那一个 `entry` 说成几行；`board` 是读到的状态文件，只在桥在跑、进程号对得上时用。
pub fn describe(entry: &Value, board: Option<&Value>) -> Vec<Line> {
    let number = |key: &str| entry[key].as_u64().unwrap_or_default();
    let state = entry["state"].as_str().unwrap_or_default();
    let mut lines = vec![Line::Say(match state {
        "off" => Report::Off,
        "starting" => Report::Starting,
        "running" => Report::Running(number("pid")),
        "waiting" => Report::Waiting {
            seconds: number("retry_in").div_ceil(1000),
            failures: number("failures"),
        },
        "stopped" => Report::Halted(halt(entry)),
        other => Report::Other(other.to_string()),
    })];
    match state {
        "running" => {
            if let Some(board) =
                board.filter(|board| entry["pid"].is_u64() && board["pid"] == entry["pid"])
            {
                lines.extend(napcat(board).into_iter().map(Line::Say));
            }
        }
        "stopped" => {
            let stderr = entry["stderr"].as_str().unwrap_or_default();
            if !stderr.trim().is_empty() {
                lines.push(Line::Say(Report::Stderr));
                lines.extend(stderr.lines().map(|line| Line::Tail(line.to_string())));
            }
        }
        _ => {}
    }
    lines
}

/// 停下的那一个为什么停下。
fn halt(entry: &Value) -> Halt {
    match entry["reason"].as_str().unwrap_or_default() {
        "config_error" => Halt::ConfigError,
        "failed_repeatedly" => {
            Halt::FailedRepeatedly(entry["failures"].as_u64().unwrap_or_default())
        }
        "not_installed" => Halt::NotInstalled,
        "cannot_start" => Halt::CannotStart,
        "protocol_mismatch" => Halt::ProtocolMismatch,
        other => Halt::Other(other.to_string()),
    }
}

/// 状态文件里的 NapCat、NapCat 那边的地址；读不懂的不说。
fn napcat(board: &Value) -> Vec<Report> {
    let napcat = &board["napcat"];
    let Some(connected) = napcat["connected"].as_bool() else {
        return Vec::new();
    };
    let Some(listen) = board["listen"].as_u64() else {
        return Vec::new();
    };
    let text = |key: &str| napcat[key].as_str().map(str::to_string);
    let first = match (connected, text("self_id")) {
        (false, _) => Report::NoNapcat,
        (true, bot) => {
            let bot = bot.unwrap_or_default();
            match (text("implementation"), text("version")) {
                (Some(implementation), Some(version)) => Report::Napcat {
                    implementation,
                    version,
                    bot,
                },
                _ => Report::NapcatBot(bot),
            }
        }
    };
    vec![first, Report::Listen(listen)]
}

/// 印一行；印不出来也没有别处可说了。
fn say(to: &mut dyn Write, line: &str) {
    if writeln!(to, "{line}").is_err() {
        // 标准输出、标准错误关了：没有别处可说。
    }
}

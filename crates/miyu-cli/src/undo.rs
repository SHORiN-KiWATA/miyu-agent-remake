//! `miyu undo`（`miyu rewind`）、`miyu restore`（`docs/designs/22-命令行.md` 第三节，施工 4-7 下，改名施工 4-7 补）：连上核心（没在跑就拉起来：撤销、恢复
//! 用不着模型），找当前会话（和 `miyu ask --continue` 一样，最新的那个一次性会话；`--session` 指定别的），撤掉最后
//! 一轮或者恢复最近一次撤销，照核心交回的几样印出改回了哪些文件（`undo/print.rs`）。
//!
//! 结果走标准输出；标准输出是终端、没设 `NO_COLOR` 才上色。被接受的退出码 0，有文件没动也是 0；被拒绝的 1。

mod print;

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use clap::Args;
use serde_json::json;

use miyu_ipc::{ConnectError, Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::{self, say, write};

/// `miyu undo`、`miyu restore` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Undo {
    /// 哪个会话；不写的是上一次 `miyu ask` 开的那个。
    #[arg(short = 's', long)]
    pub session: Option<String>,
}

/// 撤销还是恢复。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// `miyu undo`：撤掉最后一轮。
    Undo,
    /// `miyu restore`：恢复最近一次撤销。
    Restore,
}

/// 这一次撤什么、怎么印。
#[derive(Debug, Clone)]
pub struct UndoPlan {
    /// 撤销还是恢复。
    pub direction: Direction,
    /// 哪个会话；没有的是最新的那个一次性会话。
    pub session: Option<String>,
    /// 界面语言。
    pub language: Language,
    /// 家目录：路径写成 `~/…`。
    pub home: Option<PathBuf>,
    /// 上不上色。
    pub color: bool,
}

/// 跑一次 `miyu undo`（`miyu restore`），交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn undo(args: Undo, direction: Direction, start: impl FnOnce() -> Command) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(exit::ERROR);
        }
    };
    ExitCode::from(runtime.block_on(run(args, direction, start)))
}

/// 在运行时里：找数据根、连上核心、撤。
async fn run(args: Undo, direction: Direction, start: impl FnOnce() -> Command) -> u8 {
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return failed(&error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return failed(&error.to_string());
    }
    // 照 `miyu ask` 的规矩（施工 4-9 再补一）：没设 key 的，核心在跑的照样连，没在跑的不拉起。撤销本身用不着
    // 模型，不拉起是怕拉起一个没有 key 的核心：之后设了 key 的 `miyu ask` 连上它，也用不了。
    let key = std::env::var("DEEPSEEK_API_KEY").is_ok_and(|key| !key.trim().is_empty());
    let connected = match key {
        true => connect_or_start(&root, start)
            .await
            .map_err(|error| error.to_string()),
        false => match miyu_ipc::connect(&root).await {
            Ok(connected) => Ok(connected),
            Err(ConnectError::NotRunning) => {
                say(&mut io::stderr(), language::current().undo_needs_key());
                return exit::NO_MODEL;
            }
            Err(error) => Err(error.to_string()),
        },
    };
    let (connection, token) = match connected {
        Ok(connected) => connected,
        Err(reason) => return failed(&reason),
    };
    let mut out = io::stdout();
    let plan = UndoPlan {
        direction,
        session: args.session,
        language: language::current(),
        home: env.home.clone(),
        color: shown::colored(out.is_terminal(), std::env::var_os("NO_COLOR").as_deref()),
    };
    undo_on(connection, &token, &plan, &mut out, &mut io::stderr()).await
}

/// 在一条连上了的连接上撤一次（恢复一次）：握手、找会话、发命令、照回应印，交回退出码。测试照它在进程里走一遍。
pub async fn undo_on(
    connection: Connection,
    token: &str,
    plan: &UndoPlan,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut rpc = Rpc::new(connection, "undo");
    let language = &plan.language;
    if let Err(code) = link::hello(&mut rpc, token, language, false, err).await {
        return code;
    }
    let session = match &plan.session {
        Some(session) => session.clone(),
        None => match link::latest_oneshot(&mut rpc, language, err).await {
            Ok(session) => session,
            Err(code) => return code,
        },
    };
    let method = match plan.direction {
        Direction::Undo => "session.revert",
        Direction::Restore => "session.unrevert",
    };
    let params = json!({"session": session});
    match link::request(&mut rpc, method, params, language, err).await {
        Ok(result) => {
            for line in print::lines(&result, plan) {
                write(out, &line.paint(plan.color));
            }
            exit::OK
        }
        Err(code) => code,
    }
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

#[cfg(test)]
mod tests;

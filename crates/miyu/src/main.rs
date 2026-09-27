//! 主程序 `miyu`（`docs/designs/12-进程形态与分发.md` 第三节，施工 3-9 上）：一个程序，像 busybox 那样按子命令
//! 分发。现在只有 `miyu core`：核心进程，由头拉起，不写进帮助。`miyu ask` 在施工 3-9（下）。
//!
//! 不认识的子命令就报错，退出码 2，绝不当成对话发给核心（R4，`22-命令行.md` 第二节）。

mod language;

use std::process::ExitCode;
use std::time::Duration;

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{Parser, Subcommand};

/// 退出码：用法不对（`22-命令行.md` 第二节）。
const USAGE: u8 = 2;

/// `miyu`。
#[derive(Parser)]
#[command(name = "miyu", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

/// 子命令。
#[derive(Subcommand)]
enum Command {
    /// 核心进程：由头拉起，平时不用人敲。
    #[command(hide = true)]
    Core {
        /// 空闲多少秒以后退出。
        #[arg(long, hide = true)]
        idle_seconds: Option<u64>,
    },
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return refused(error),
    };
    match cli.command {
        Some(Command::Core { idle_seconds }) => miyu_core::main(miyu_core::Options {
            idle: idle_seconds.map_or(miyu_core::IDLE, Duration::from_secs),
        }),
        None => {
            eprintln!("{}", language::current().nothing_yet());
            ExitCode::from(USAGE)
        }
    }
}

/// 参数不对：不认识的子命令照 22 第二节的话说；帮助、版本照常印；别的交给 clap 说。
#[expect(
    clippy::let_underscore_must_use,
    reason = "帮助、报错印不出来，也没有别处可说了"
)]
fn refused(error: clap::Error) -> ExitCode {
    match error.kind() {
        ErrorKind::InvalidSubcommand => {
            let name = match error.get(ContextKind::InvalidSubcommand) {
                Some(ContextValue::String(name)) => name.as_str(),
                _ => "",
            };
            eprintln!("{}", language::current().no_such_command(name));
            ExitCode::from(USAGE)
        }
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            let _ = error.print();
            ExitCode::SUCCESS
        }
        _ => {
            let _ = error.print();
            ExitCode::from(USAGE)
        }
    }
}

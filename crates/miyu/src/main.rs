//! 主程序 `miyu`（`docs/designs/12-进程形态与分发.md` 第三节，施工 3-9）：一个程序，像 busybox 那样按子命令
//! 分发。`miyu ask` 是最薄的头（施工 3-9 下）；`miyu undo`、`miyu redo` 撤掉最后一轮、恢复（施工 4-7 下）；
//! `miyu core` 是核心进程，由头拉起，不写进帮助。
//!
//! 不认识的子命令就报错，退出码 2，绝不当成对话发给核心（R4，`22-命令行.md` 第二节）。

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

use miyu_cli::{Direction, language};

/// 退出码：用法不对（`22-命令行.md` 第二节）。
const USAGE: u8 = 2;

/// 主程序的参数。
#[derive(Parser)]
#[command(name = "miyu", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

/// 子命令。
#[derive(Subcommand)]
enum Command {
    /// 说一句话，打印她的回答。
    Ask(miyu_cli::Ask),
    /// 撤掉当前会话的最后一轮，把她改过的文件改回去。
    Undo(miyu_cli::Undo),
    /// 发下一句之前，恢复最近一次撤销。
    Redo(miyu_cli::Undo),
    /// 核心进程：由头拉起，平时不用人敲。
    #[command(hide = true)]
    Core {
        /// 空闲多少秒以后退出。
        #[arg(long, hide = true)]
        idle_seconds: Option<u64>,
    },
}

fn main() -> ExitCode {
    let language = language::current();
    // `--help` 没有说明那一行，直接从用法开始：clap 会把这里、子命令枚举上的文档注释当成说明印出来（施工 4-9 再补四上：
    // 原来印的是「`miyu`。」）。说明是产品的话，等做终端界面那一步定。
    let command = Cli::command()
        .about(None::<&str>)
        .long_about(None::<&str>)
        .mut_subcommand("ask", |ask| miyu_cli::localize(ask, &language))
        .mut_subcommand("undo", |undo| {
            miyu_cli::localize_undo(undo, &language, Direction::Undo)
        })
        .mut_subcommand("redo", |redo| {
            miyu_cli::localize_undo(redo, &language, Direction::Redo)
        });
    let cli = match command
        .try_get_matches()
        .and_then(|matches| Cli::from_arg_matches(&matches))
    {
        Ok(cli) => cli,
        Err(error) => return refused(error),
    };
    match cli.command {
        Some(Command::Ask(args)) => miyu_cli::ask(args, core),
        Some(Command::Undo(args)) => miyu_cli::undo(args, Direction::Undo, core),
        Some(Command::Redo(args)) => miyu_cli::undo(args, Direction::Redo, core),
        Some(Command::Core { idle_seconds }) => miyu_core::main(miyu_core::Options {
            idle: idle_seconds.map_or(miyu_core::IDLE, Duration::from_secs),
        }),
        None => {
            eprintln!("{}", language::current().nothing_yet());
            ExitCode::from(USAGE)
        }
    }
}

/// 拉起核心的命令：自己这个程序，加上 `core`。
fn core() -> std::process::Command {
    let mut core = std::process::Command::new(
        std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu")),
    );
    core.arg("core");
    core
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

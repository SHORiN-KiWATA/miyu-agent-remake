//! 主程序 `miyu`（`docs/designs/12-进程形态与分发.md` 第三节，施工 3-9）：一个程序，像 busybox 那样按子命令
//! 分发。`miyu ask` 是最薄的头（施工 3-9 下）；`miyu undo`、`miyu redo` 撤掉最后一轮、恢复（施工 4-7 下）；
//! `miyu sandbox setup`、`remove` 在 Windows 上装好、撤掉沙盒用户（施工 5-8）；`miyu core` 是核心进程，由头拉起，
//! 不写进帮助。
//!
//! 不认识的子命令就报错，退出码 2，绝不当成对话发给核心（R4，`22-命令行.md` 第二节）。帮助页、参数写错时说的
//! 那一句都是自己写的，跟着界面语言（施工 4-11，`docs/blueprint/cli/main.md`）。

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::error::ErrorKind;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

use miyu_cli::help::{Page, page};
use miyu_cli::language::{self, Language};
use miyu_cli::{Direction, misuse};

/// 退出码：用法不对（`22-命令行.md` 第二节）。
const USAGE: u8 = 2;

/// 主程序的参数。名字定死成 `miyu`：clap 默认照可执行文件的名字，Windows 上会在用法和报错里带出 `miyu.exe`（施工 5-8
/// 查出来的）。
#[derive(Parser)]
#[command(name = "miyu", bin_name = "miyu", version)]
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
    /// 装好、撤掉沙盒用户（Windows，要管理员权限）。
    Sandbox(miyu_cli::Sandbox),
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
    // 帮助页换成自己写的（施工 4-11）：`-h`、`--help`、`miyu help <子命令>` 都印它们，clap 生成的一个字都不印。
    let command = Cli::command()
        .override_help(page(language, Page::Miyu))
        .mut_subcommand("ask", |ask| ask.override_help(page(language, Page::Ask)))
        .mut_subcommand("undo", |undo| {
            undo.override_help(page(language, Page::Undo))
        })
        .mut_subcommand("redo", |redo| {
            redo.override_help(page(language, Page::Redo))
        })
        .mut_subcommand("sandbox", |sandbox| {
            let help = page(language, Page::Sandbox);
            sandbox
                .override_help(help)
                .mut_subcommand("setup", |setup| setup.override_help(help))
                .mut_subcommand("remove", |remove| remove.override_help(help))
        });
    let cli = match command
        .try_get_matches()
        .and_then(|matches| Cli::from_arg_matches(&matches))
    {
        Ok(cli) => cli,
        Err(error) => return refused(error, language),
    };
    match cli.command {
        Some(Command::Ask(args)) => miyu_cli::ask(args, core),
        Some(Command::Undo(args)) => miyu_cli::undo(args, Direction::Undo, core),
        Some(Command::Redo(args)) => miyu_cli::undo(args, Direction::Redo, core),
        Some(Command::Sandbox(args)) => miyu_cli::sandbox(args),
        Some(Command::Core { idle_seconds }) => miyu_core::main(miyu_core::Options {
            idle: idle_seconds.map_or(miyu_core::IDLE, Duration::from_secs),
        }),
        None => {
            eprintln!("{}", language.nothing_yet());
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

/// 参数不对：帮助、版本照常印在标准输出上，退出码 0；别的在标准错误上说一句（不认识的子命令照 22 第二节的话说），
/// 退出码 2。
#[expect(
    clippy::let_underscore_must_use,
    reason = "帮助印不出来，也没有别处可说了"
)]
fn refused(error: clap::Error, language: Language) -> ExitCode {
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            let _ = error.print();
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{}", misuse(&error, language));
            ExitCode::from(USAGE)
        }
    }
}

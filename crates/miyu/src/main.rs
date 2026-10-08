//! 主程序 `miyu`（`docs/designs/12-进程形态与分发.md` 第三节，施工 3-9）：一个程序，像 busybox 那样按子命令
//! 分发。`miyu ask` 是最薄的头（施工 3-9 下）；`miyu undo`（`miyu rewind`）、`miyu restore` 撤掉最后一轮、恢复（施工 4-7 下，改名施工 4-7 补）；
//! `miyu redo` 重做最后一轮（施工 4-7 再补）；
//! `miyu compact` 手动压缩（施工 6-8）；`miyu recap` 一句话回顾（施工 3-8 四补）；`miyu rename` 给会话起名（施工 3-8 五补）；`miyu config` 看配置（施工 8-2）；`miyu login`、`miyu logout` 管 key（施工 8-5）；`miyu setup` 接上第一个模型（施工 8-11）；`miyu sandbox setup`、`remove` 在 Windows 上装好、撤掉沙盒用户（施工 5-8）；`miyu core` 是核心进程，由头拉起，
//! 不写进帮助。
//!
//! 不认识的子命令就报错，退出码 2，绝不当成对话发给核心（R4，`22-命令行.md` 第二节）。帮助页、参数写错时说的
//! 那一句都是自己写的，跟着界面语言（施工 4-11，`docs/blueprint/cli/main.md`）。

use std::ffi::OsString;
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
    /// 撤掉当前会话的最后一轮，把她改过的文件改回去。也可以写成 `rewind`（施工 4-7 补）。
    #[command(alias = "rewind")]
    Undo(miyu_cli::Undo),
    /// 发下一句之前，恢复最近一次撤销（原来叫 `redo`，施工 4-7 补改名）。
    Restore(miyu_cli::Undo),
    /// 撤掉最后一轮，把人那句话（或者换成写的话）再发一次，让她重新做（施工 4-7 再补）。
    Redo(miyu_cli::Redo),
    /// 把当前会话的上下文压缩成摘要，可以附上要求（施工 6-8）。
    Compact(miyu_cli::Compact),
    /// 一句话回顾当前会话：在做什么、做完了什么、卡在哪（施工 3-8 四补）。
    Recap(miyu_cli::Recap),
    /// 给当前会话起名（施工 3-8 五补）。
    Rename(miyu_cli::Rename),
    /// 装好、撤掉沙盒用户（Windows，要管理员权限）。
    Sandbox(miyu_cli::Sandbox),
    /// 看配置、改配置、信任项目配置（施工 8-2、8-3）。
    Config(miyu_cli::Config),
    /// 查手写的文件有没有写错：配置、密钥文件、人格（施工 8-30）。
    Check(miyu_cli::Check),
    /// 存一个供应商的 key，`--list` 列出哪几个设了（施工 8-5）。
    Login(miyu_cli::Login),
    /// 删掉一个供应商的 key（施工 8-5）。
    Logout(miyu_cli::Logout),
    /// 第一次接入模型（施工 8-11）。
    Setup(miyu_cli::Setup),
    /// 打开网页界面：找主程序旁边的网页软件，参数交给它（施工 W-9）。
    Web(miyu_cli::Web),
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
    // 软件包加的子命令（施工 9-2）：第一个词不是内置的（不认识的、`help`、选项）才照清单找，`miyu core`、`miyu ask` 不多读盘。
    let args: Vec<OsString> = std::env::args_os().collect();
    let builtins = builtins();
    let first = args.get(1).and_then(|first| first.to_str());
    let added = match first {
        Some(first) if first != "help" && builtins.iter().any(|name| name == first) => Vec::new(),
        _ => {
            let names: Vec<&str> = builtins.iter().map(String::as_str).collect();
            miyu_cli::packages::installed(&miyu_core::admin(), &names)
        }
    };
    let main = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu"));
    if let Some(code) =
        miyu_cli::packages::forward(&args, &added, &main, language, &mut std::io::stderr())
    {
        return ExitCode::from(code);
    }
    let section = miyu_cli::packages::help_section(language, &added);
    let help = miyu_cli::packages::with_section(page(language, Page::Miyu), &section);
    // 帮助页换成自己写的（施工 4-11）：`-h`、`--help`、`miyu help <子命令>` 都印它们，clap 生成的一个字都不印。
    let command = Cli::command()
        .override_help(help.clone())
        .mut_subcommand("ask", |ask| ask.override_help(page(language, Page::Ask)))
        .mut_subcommand("undo", |undo| {
            undo.override_help(page(language, Page::Undo))
        })
        .mut_subcommand("restore", |restore| {
            restore.override_help(page(language, Page::Restore))
        })
        .mut_subcommand("redo", |redo| {
            redo.override_help(page(language, Page::Redo))
        })
        .mut_subcommand("compact", |compact| {
            compact.override_help(page(language, Page::Compact))
        })
        .mut_subcommand("recap", |recap| {
            recap.override_help(page(language, Page::Recap))
        })
        .mut_subcommand("rename", |rename| {
            rename.override_help(page(language, Page::Rename))
        })
        .mut_subcommand("login", |login| {
            login.override_help(page(language, Page::Login))
        })
        .mut_subcommand("logout", |logout| {
            logout.override_help(page(language, Page::Logout))
        })
        .mut_subcommand("setup", |setup| {
            setup.override_help(page(language, Page::Setup))
        })
        .mut_subcommand("web", |web| web.override_help(page(language, Page::Web)))
        .mut_subcommand("check", |check| {
            check.override_help(page(language, Page::Check))
        })
        .mut_subcommand("config", |config| {
            let help = page(language, Page::Config);
            ["get", "explain", "path", "set", "unset", "edit", "trust"]
                .into_iter()
                .fold(config.override_help(help), |config, name| {
                    config.mut_subcommand(name, |sub| sub.override_help(help))
                })
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
        Some(Command::Restore(args)) => miyu_cli::undo(args, Direction::Restore, core),
        Some(Command::Redo(args)) => miyu_cli::redo(args, core),
        Some(Command::Compact(args)) => miyu_cli::compact(args, core),
        Some(Command::Recap(args)) => miyu_cli::recap(args, core),
        Some(Command::Rename(args)) => miyu_cli::rename(args, core),
        Some(Command::Sandbox(args)) => miyu_cli::sandbox(args),
        // 不带子命令、在终端里：打开界面的设置页（施工 9-3，原来的 8-24）；界面不认设置页的照旧印帮助。
        Some(Command::Config(args)) if args.command.is_none() && terminal() => {
            match miyu_cli::head::open(Some("config"), &miyu_core::admin(), core) {
                miyu_cli::head::Opened::Ran(code) => ExitCode::from(code),
                miyu_cli::head::Opened::NoPage => miyu_cli::config(args, core),
            }
        }
        Some(Command::Config(args)) => miyu_cli::config(args, core),
        Some(Command::Check(args)) => miyu_cli::check(args, core),
        Some(Command::Login(args)) => miyu_cli::login(args.into(), core),
        Some(Command::Logout(args)) => miyu_cli::login(args.into(), core),
        Some(Command::Setup(args)) => miyu_cli::setup(args, core),
        Some(Command::Web(args)) => miyu_cli::web(args, &miyu_core::admin()),
        Some(Command::Core { idle_seconds }) => miyu_core::main(miyu_core::Options {
            idle: idle_seconds.map_or(miyu_core::IDLE, Duration::from_secs),
        }),
        // 不带子命令（施工 9-3）：在终端里打开 `ui.head` 指的界面；不在终端里（被脚本调、接管道）印帮助、退出码 2。
        None if terminal() => match miyu_cli::head::open(None, &miyu_core::admin(), core) {
            miyu_cli::head::Opened::Ran(code) => ExitCode::from(code),
            miyu_cli::head::Opened::NoPage => ExitCode::from(USAGE),
        },
        None => {
            print!("{help}");
            ExitCode::from(USAGE)
        }
    }
}

/// 标准输入、标准输出都是终端（施工 9-3）：只有这时才拉起界面。
fn terminal() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// 内置的子命令（施工 9-2）：每一个的名字、别名，加上 `help`。软件包的子命令撞了它们的不转交、不列。
fn builtins() -> Vec<String> {
    let command = Cli::command();
    command
        .get_subcommands()
        .flat_map(|sub| {
            std::iter::once(sub.get_name().to_string())
                .chain(sub.get_all_aliases().map(str::to_string))
        })
        .chain(std::iter::once("help".to_string()))
        .collect()
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

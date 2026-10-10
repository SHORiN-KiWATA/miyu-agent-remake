//! `miyu pkg`（`docs/blueprint/cli/pkg.md`，施工 T-3）：在终端里列出软件包、从一份清单装、把卸掉的出厂的装回来、卸掉。连上
//! 核心（照 `miyu memory`：没在跑就拉起来），发 `package.*`（`protocol.md`），照回应印一句或者一行一个。
//!
//! 做成了退出码 0；核心拒的照核心的原话印在标准错误上（清单装不上的照 `data` 说哪里不对），退出码 1；参数写错的 2，由 clap 管。

mod shown;

use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use miyu_ipc::{Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::ask::Format;
use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::say;

/// `miyu pkg` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Pkg {
    /// 哪个子命令；不写的是 `list`。
    #[command(subcommand)]
    pub command: Option<PkgCommand>,
    /// `list` 输出的格式。
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

/// `miyu pkg` 的子命令。
#[derive(Debug, Clone, Subcommand)]
pub enum PkgCommand {
    /// 列出来，照编号。
    List,
    /// 装：一份清单的路径（带 `/`、`\` 或者以 `.toml` 结尾的），或者卸掉了的出厂的包的编号。
    Install {
        /// 装哪一份。
        what: String,
    },
    /// 卸掉。
    Remove {
        /// 卸哪一个。
        package: String,
    },
}

/// 这一次做什么，照哪种语言说，相对的路径照哪个目录算。
#[derive(Debug, Clone)]
pub struct PkgPlan {
    /// 读好的参数。
    pub args: Pkg,
    /// 界面语言。
    pub language: Language,
    /// 现在的工作目录：`install` 写的相对路径照它换成绝对的（核心只收绝对的）。
    pub cwd: std::path::PathBuf,
}

/// 跑一次 `miyu pkg`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn pkg(args: Pkg, start: impl FnOnce() -> Command) -> ExitCode {
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
    ExitCode::from(runtime.block_on(run(args, start)))
}

/// 在运行时里：找数据根、连上核心、照子命令做。
async fn run(args: Pkg, start: impl FnOnce() -> Command) -> u8 {
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return failed(&error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return failed(&error.to_string());
    }
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(error) => return failed(&error.to_string()),
    };
    let connected = connect_or_start(&root, start)
        .await
        .map_err(|error| error.to_string());
    let (connection, token) = match connected {
        Ok(connected) => connected,
        Err(reason) => return failed(&reason),
    };
    let plan = PkgPlan {
        args,
        language: language::current(),
        cwd,
    };
    pkg_on(
        connection,
        &token,
        &plan,
        &mut io::stdout(),
        &mut io::stderr(),
    )
    .await
}

/// 在一条连上了的连接上做一次：握手、照子命令发、印，交回退出码；印的写在 `out`，出错的写在 `err`。测试照它在进程里走一遍。
pub async fn pkg_on(
    connection: Connection,
    token: &str,
    plan: &PkgPlan,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut rpc = Rpc::new(connection, "pkg");
    let language = &plan.language;
    if let Err(code) = link::hello(&mut rpc, token, language, false, err).await {
        return code;
    }
    match act(&mut rpc, plan, out, err).await {
        Ok(()) => exit::OK,
        Err(code) => code,
    }
}

/// 照子命令发一条 `package.*`，照回应印。
async fn act(
    rpc: &mut Rpc,
    plan: &PkgPlan,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let language = &plan.language;
    let command = plan.args.command.clone().unwrap_or(PkgCommand::List);
    let (method, params) = match &command {
        PkgCommand::List => ("package.list", json!({})),
        PkgCommand::Install { what } => ("package.install", installing(what, &plan.cwd)),
        PkgCommand::Remove { package } => ("package.remove", json!({"package": package})),
    };
    let result = link::request_saying(rpc, method, params, language, err, |error| {
        refused(error, language)
    })
    .await?;
    let said = match command {
        PkgCommand::List => return print(&result, plan, out),
        PkgCommand::Install { .. } => language.installed(package_of(&result)),
        PkgCommand::Remove { .. } => language.uninstalled(package_of(&result)),
    };
    say(out, &said);
    Ok(())
}

/// `install` 装的是什么：带 `/`、`\`，是 `.`、`..`，或以 `.toml` 结尾的是一个包目录（或它里面的清单），换成绝对路径（相对的
/// 照 `cwd` 算，施工 F-8 上）；别的是卸掉了的出厂的包的编号。
fn installing(what: &str, cwd: &Path) -> Value {
    let path = what.contains('/')
        || what.contains('\\')
        || what == "."
        || what == ".."
        || what.ends_with(".toml");
    match path {
        true => json!({"path": joined(cwd, what)}),
        false => json!({"package": what}),
    }
}

/// 相对的照 `cwd` 接成绝对的，`.`、`..` 照字面去掉（不碰磁盘）：核心照最后一段认编号，`.` 认不出来。
fn joined(cwd: &Path, what: &str) -> PathBuf {
    let mut path = PathBuf::new();
    for part in cwd.join(what).components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                path.pop();
            }
            other => path.push(other),
        }
    }
    path
}

/// `list` 的回应印出来：`json` 的原样印那一串；`text` 的一行一个。
fn print(result: &Value, plan: &PkgPlan, out: &mut dyn Write) -> Result<(), u8> {
    let packages = result["packages"].as_array().cloned().unwrap_or_default();
    if plan.args.format == Format::Json {
        say(out, &Value::Array(packages).to_string());
        return Ok(());
    }
    for row in shown::listed(&packages, &plan.language) {
        say(out, &row);
    }
    Ok(())
}

/// 被拒绝时说的那一句：清单装不上的照 `data` 说哪里不对、第几行（核心的原话是给程序看的，说的是去 `data` 里看）；别的照
/// 核心的原话。
fn refused(error: &Value, language: &Language) -> String {
    let data = &error["data"];
    match (data["reason"].as_str(), data["problem"].as_str()) {
        (Some("package_invalid"), Some(problem)) => {
            language.cannot_install(problem, data["line"].as_u64())
        }
        _ => language.refused(error["message"].as_str().unwrap_or_default()),
    }
}

fn package_of(result: &Value) -> &str {
    result["package"].as_str().unwrap_or_default()
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

#[cfg(test)]
mod tests;

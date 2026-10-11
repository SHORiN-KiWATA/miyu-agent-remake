//! `miyu pkg`（`docs/blueprint/cli/pkg.md`，施工 T-3）：在终端里列出软件包、从一份清单装、把卸掉的出厂的装回来、卸掉；看一个包
//! 的信息、装了哪些文件、一个文件归哪个包、装好的文件改没改（施工 F-8 下）。连上核心（照 `miyu memory`：没在跑就拉起来），发
//! `package.*`（`protocol.md`），照回应印一句或者一行一个。pacman 的 `-U`、`-R`、`-Q`、`-Qi`、`-Ql`、`-Qo`、`-Qk` 当别名也认。
//!
//! 做成了退出码 0；核心拒的照核心的原话印在标准错误上（清单装不上的照 `data` 说哪里不对），退出码 1；`owns` 哪个包都没有的、
//! `check` 查出改了少了的也是 1（照 pacman）；参数写错的 2，由 clap 管。

mod confirm;
mod query;
mod shown;

use std::io::{self, BufRead, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use miyu_ipc::{Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::ask::Format;
use crate::exit;
use crate::language::{self, Doing, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::{offset, say};

/// `miyu pkg` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Pkg {
    /// 哪个子命令；不写的是 `list`。
    #[command(subcommand)]
    pub command: Option<PkgCommand>,
    /// `list`、`info`、`files`、`owns`、`check` 输出的格式。
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    pub format: Format,
    /// 装、卸之前不问（施工 F-8 下补）；`--noconfirm` 是 pacman 的写法。
    #[arg(long, visible_alias = "noconfirm", global = true)]
    pub yes: bool,
}

/// `miyu pkg` 的子命令。
#[derive(Debug, Clone, Subcommand)]
pub enum PkgCommand {
    /// 列出来，照编号。
    List,
    /// 装：一份清单的路径（带 `/`、`\` 或者以 `.toml` 结尾的），或者卸掉了的出厂的包的编号。pacman 的 `-U`。
    #[command(short_flag = 'U')]
    Install {
        /// 装哪一份。
        what: String,
    },
    /// 卸掉。pacman 的 `-R`。
    #[command(short_flag = 'R')]
    Remove {
        /// 卸哪一个。
        package: String,
    },
    /// 一个包的信息。
    Info {
        /// 哪一个。
        package: String,
    },
    /// 一个包装了哪些文件。
    Files {
        /// 哪一个。
        package: String,
    },
    /// 一个文件归哪个包。
    Owns {
        /// 哪个文件，相对的照现在的目录算。
        path: String,
    },
    /// 装好的文件改没改；不写的是家目录里记了的全部。
    Check {
        /// 只查哪一个。
        package: Option<String>,
    },
    /// pacman 的 `-Q`：不带别的是 `list`，`-i` 是 `info`，`-l` 是 `files`，`-o` 是 `owns`，`-k` 是 `check`。帮助页里写，
    /// clap 的列表里不列。
    #[command(short_flag = 'Q', hide = true)]
    Query(Query),
}

/// `-Q` 后面接的：几个字母最多一个，`-i`、`-l`、`-o` 要接一个包或者路径。
#[derive(Debug, Clone, Args)]
pub struct Query {
    /// `-Qi`：`info`。
    #[arg(short = 'i', group = "query", requires = "target")]
    info: bool,
    /// `-Ql`：`files`。
    #[arg(short = 'l', group = "query", requires = "target")]
    files: bool,
    /// `-Qo`：`owns`。
    #[arg(short = 'o', group = "query", requires = "target")]
    owns: bool,
    /// `-Qk`：`check`。
    #[arg(short = 'k', group = "query")]
    check: bool,
    /// 包的编号，`-Qo` 的是路径。
    #[arg(requires = "query")]
    target: Option<String>,
}

impl PkgCommand {
    /// pacman 的写法换成正式的子命令。
    fn plain(self) -> PkgCommand {
        let PkgCommand::Query(query) = self else {
            return self;
        };
        let target = query.target.unwrap_or_default();
        match (query.info, query.files, query.owns, query.check) {
            (true, ..) => PkgCommand::Info { package: target },
            (_, true, ..) => PkgCommand::Files { package: target },
            (_, _, true, _) => PkgCommand::Owns { path: target },
            (.., true) => PkgCommand::Check {
                package: (!target.is_empty()).then_some(target),
            },
            _ => PkgCommand::List,
        }
    }
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
        &mut io::stdin().lock(),
        &mut io::stdout(),
        &mut io::stderr(),
    )
    .await
}

/// 在一条连上了的连接上做一次：握手、照子命令发、印，交回退出码；装、卸之前问的答从 `input` 读，印的写在 `out`，出错的写在
/// `err`。测试照它在进程里走一遍。
pub async fn pkg_on(
    connection: Connection,
    token: &str,
    plan: &PkgPlan,
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut rpc = Rpc::new(connection, "pkg");
    let language = &plan.language;
    if let Err(code) = link::hello(&mut rpc, token, language, false, err).await {
        return code;
    }
    match act(&mut rpc, plan, (input, out, err)).await {
        Ok(()) => exit::OK,
        Err(code) => code,
    }
}

/// 照子命令发 `package.*`，照回应印。
async fn act(
    rpc: &mut Rpc,
    plan: &PkgPlan,
    (input, out, err): (&mut dyn BufRead, &mut dyn Write, &mut dyn Write),
) -> Result<(), u8> {
    let language = &plan.language;
    let json = plan.args.format == Format::Json;
    match plan
        .args
        .command
        .clone()
        .unwrap_or(PkgCommand::List)
        .plain()
    {
        PkgCommand::List | PkgCommand::Query(_) => {
            let result = ask(rpc, "package.list", json!({}), language, err).await?;
            print(&result, plan, out)
        }
        PkgCommand::Install { what } => {
            let params = installing(&what, &plan.cwd);
            if !plan.args.yes {
                let doing = match params.get("path") {
                    Some(_) => Doing::Install,
                    None => Doing::Restore,
                };
                let shown = ask(rpc, "package.install", previewed(&params), language, err).await?;
                lines(out, confirm::plan(&shown, doing, language));
                confirm::asked(input, out, err, doing, language)?;
            }
            let result = ask(rpc, "package.install", params, language, err).await?;
            say(out, &language.installed(package_of(&result)));
            Ok(())
        }
        PkgCommand::Remove { package } => {
            let params = json!({"package": package});
            if !plan.args.yes {
                let shown = ask(rpc, "package.remove", previewed(&params), language, err).await?;
                lines(out, confirm::plan(&shown, Doing::Remove, language));
                confirm::asked(input, out, err, Doing::Remove, language)?;
            }
            let result = ask(rpc, "package.remove", params, language, err).await?;
            say(out, &language.uninstalled(package_of(&result)));
            Ok(())
        }
        PkgCommand::Info { package } => {
            let params = json!({"package": package});
            let info = ask(rpc, "package.info", params, language, err).await?;
            if json {
                say(out, &info.to_string());
                return Ok(());
            }
            let listed = ask(rpc, "package.list", json!({}), language, err).await?;
            let name = listed["packages"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|one| one["package"] == package.as_str())
                .and_then(|one| one["name"].as_str())
                .unwrap_or_default();
            lines(out, query::info(&info, name, offset(), language));
            Ok(())
        }
        PkgCommand::Files { package } => {
            let params = json!({"package": package});
            let files = ask(rpc, "package.files", params, language, err).await?;
            match json {
                true => say(out, &files.to_string()),
                false => lines(out, query::files(&package, &files)),
            }
            Ok(())
        }
        PkgCommand::Owns { path } => {
            let path = joined(&plan.cwd, &path);
            let params = json!({"path": path});
            let owned = ask(rpc, "package.owns", params, language, err).await?;
            let shown = path.display().to_string();
            match (json, query::owner(&shown, &owned, language)) {
                (true, found) => {
                    say(out, &owned.to_string());
                    found.map(drop).ok_or(exit::ERROR)
                }
                (false, Some(line)) => {
                    say(out, &line);
                    Ok(())
                }
                (false, None) => {
                    say(err, &language.owned_by_none(&shown));
                    Err(exit::ERROR)
                }
            }
        }
        PkgCommand::Check { package } => {
            let params = package.map_or_else(|| json!({}), |package| json!({"package": package}));
            let checked = ask(rpc, "package.check", params, language, err).await?;
            let (rows, broken) = query::checked(&checked, language);
            match json {
                true => say(out, &checked.to_string()),
                false => lines(out, rows),
            }
            if broken { Err(exit::ERROR) } else { Ok(()) }
        }
    }
}

/// 发一条请求；被拒绝的照 [`refused`] 说一句。
async fn ask(
    rpc: &mut Rpc,
    method: &str,
    params: Value,
    language: &Language,
    err: &mut dyn Write,
) -> Result<Value, u8> {
    link::request_saying(rpc, method, params, language, err, |error| {
        refused(error, language)
    })
    .await
}

/// 同一条请求，只看一眼（`preview`，施工 F-8 下补）。
fn previewed(params: &Value) -> Value {
    let mut params = params.clone();
    params["preview"] = json!(true);
    params
}

/// 一行一行印。
fn lines(out: &mut dyn Write, rows: Vec<String>) {
    for row in rows {
        say(out, &row);
    }
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
    lines(out, shown::listed(&packages, &plan.language));
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

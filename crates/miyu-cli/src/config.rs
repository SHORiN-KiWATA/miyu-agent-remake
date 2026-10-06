//! `miyu config`（`docs/blueprint/cli/config.md`，`config.md`「命令行」第十条）：看配置的四个子命令 `get`、`check`、
//! `explain`、`path`（施工 8-2），改配置、信任项目配置的四个 `set`、`unset`、`edit`、`trust`（施工 8-3）。它们只是协议的
//! 客户端（`22-命令行.md` O5）：连上核心，`config.get`、`config.schema`、`config.check`、`config.set`、`config.trust`，照回应
//! 印。`edit`、`trust` 要问人、开编辑器，经 [`Console`]：测试换成照剧本回的。
//!
//! 连核心照 `miyu recap`：核心在跑的照样连，没在跑的拉起（施工 8-6 起 key 来自配置，一律拉起）。握手以后给人看的字照回应的
//! `language`（施工 8-2）。
//!
//! 退出码：0 成了（`check` 没有错误、`edit` 没改、`unset` 本来就没写、`trust` 记下了或本来就信任着）；1 核心拒绝了、`check`
//! 有错误、`edit` 放弃了或编辑器出错或冲突、`trust` 这里没有项目配置或冲突、连不上核心；2 参数不对（在主程序里；`edit`、
//! `trust` 不在终端里又没写 `--yes`、`--no`；`set --project`）。

mod check;
mod console;
mod edit;
mod paths;
mod render;
mod set;
#[cfg(test)]
mod tests;
mod trust;

pub use console::{Console, Terminal};
pub(crate) use render::columns;

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
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
use crate::shown::{self, say, write};

/// `miyu config` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Config {
    /// 哪个子命令。
    #[command(subcommand)]
    pub command: ConfigCommand,
}

/// 参数不对（`cli/main.md`「参数写错时」）：不在终端里的 `edit`、`trust`，`set --project`（施工 8-3）。
const MISUSE: u8 = 2;

/// `miyu config` 的子命令。
#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCommand {
    /// 印出最终值。
    Get {
        /// 只要这几项；不写是全部。
        keys: Vec<String>,
        /// 输出的格式。
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
    },
    /// 检查配置有没有写错。
    Check {
        /// 查这一份文件；不写的查现在的几份。
        file: Option<PathBuf>,
        /// 当成系统配置查（只查系统配置）。
        #[arg(long, conflicts_with = "project")]
        system: bool,
        /// 当成项目配置查（只查当前目录的项目配置）。
        #[arg(long)]
        project: bool,
        /// 输出的格式。
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
    },
    /// 这一项每一层写的什么、哪一个生效。
    Explain {
        /// 哪一项。
        key: String,
        /// 输出的格式。
        #[arg(long, value_enum, default_value = "text")]
        format: Format,
    },
    /// 印出配置文件在哪。
    Path {
        /// 系统配置。
        #[arg(long, conflicts_with = "project")]
        system: bool,
        /// 当前目录的项目配置。
        #[arg(long)]
        project: bool,
    },
    /// 改一项（施工 8-3）。
    Set {
        /// 哪一项。
        key: String,
        /// 改成什么，照这一项的类型读。
        value: String,
        /// 改系统配置；不写改个人设置。
        #[arg(long, conflicts_with = "project")]
        system: bool,
        /// 项目配置只能手改：写了是参数不对，说一句怎么改。
        #[arg(long)]
        project: bool,
    },
    /// 从这一层删掉一项（施工 8-3）。
    Unset {
        /// 哪一项。
        key: String,
        /// 从系统配置删；不写从个人设置删。
        #[arg(long)]
        system: bool,
    },
    /// 用编辑器打开，存盘时先检查（施工 8-3）。
    Edit {
        /// 系统配置。
        #[arg(long, conflicts_with = "project")]
        system: bool,
        /// 当前目录的项目配置。
        #[arg(long)]
        project: bool,
    },
    /// 看当前目录的项目配置会改什么，信任或者不信任它（施工 8-3）。
    Trust {
        /// 信任，不问。
        #[arg(long, conflicts_with = "no")]
        yes: bool,
        /// 不信任，不问。
        #[arg(long)]
        no: bool,
    },
}

/// 这一次做什么、在哪、怎么印。
#[derive(Debug, Clone)]
pub struct ConfigPlan {
    /// 哪个子命令。
    pub command: ConfigCommand,
    /// 界面语言：握手以后换成回应的。
    pub language: Language,
    /// 工作目录：找项目配置照它。
    pub cwd: PathBuf,
    /// 数据根：核心报的文件是相对它的。
    pub root: PathBuf,
    /// 家目录：路径写成 `~/…`，`~/…` 照它换开。
    pub home: Option<PathBuf>,
    /// 标准输出上不上色。
    pub color: bool,
    /// 标准错误上的灰字上不上色（施工 8-3：`set`、`edit`、`trust` 印的那一行）。
    pub gray: bool,
}

/// 跑一次 `miyu config`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn config(args: Config, start: impl FnOnce() -> Command) -> ExitCode {
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

/// 在运行时里：找数据根、连上核心、照子命令办。
async fn run(args: Config, start: impl FnOnce() -> Command) -> u8 {
    let language = language::current();
    let mut console = Terminal::current();
    if let Some(code) = early(&args.command, language, &console, &mut io::stderr()) {
        return code;
    }
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return failed(&error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return failed(&error.to_string());
    }
    let connected = connect_or_start(&root, start)
        .await
        .map_err(|error| error.to_string());
    let (connection, token) = match connected {
        Ok(connected) => connected,
        Err(reason) => return failed(&reason),
    };
    let plan = ConfigPlan {
        command: args.command,
        language,
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        root: root.path().to_path_buf(),
        home: env.home.clone(),
        color: shown::colored(
            io::stdout().is_terminal(),
            std::env::var_os("NO_COLOR").as_deref(),
        ),
        gray: shown::colored(
            io::stderr().is_terminal(),
            std::env::var_os("NO_COLOR").as_deref(),
        ),
    };
    config_on(
        connection,
        &token,
        &plan,
        &mut console,
        &mut io::stdout(),
        &mut io::stderr(),
    )
    .await
}

/// 在一条连上了的连接上办一次 `miyu config`：握手，照子命令问核心、印，交回退出码。要问人、开编辑器的经 `console`。测试
/// 照它在进程里走一遍。
pub async fn config_on(
    connection: Connection,
    token: &str,
    plan: &ConfigPlan,
    console: &mut dyn Console,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    if let Some(code) = early(&plan.command, plan.language, console, err) {
        return code;
    }
    let mut rpc = Rpc::new(connection, "config");
    let plan = match link::hello(&mut rpc, token, &plan.language, false, err).await {
        Ok(hello) => ConfigPlan {
            language: link::spoken(&hello, plan.language),
            ..plan.clone()
        },
        Err(code) => return code,
    };
    let mut talk = Talk {
        rpc: &mut rpc,
        plan: &plan,
        err,
    };
    match &plan.command {
        ConfigCommand::Get { keys, format } => get(&mut talk, keys, *format, out).await,
        ConfigCommand::Explain { key, format } => {
            explain(&mut talk, &tidy(key), *format, out).await
        }
        ConfigCommand::Path { system, project } => path(&mut talk, *system, *project, out).await,
        ConfigCommand::Check {
            file,
            system,
            project,
            format,
        } => {
            let only = check::Only::of(*system, *project);
            check::check(&mut talk, file.as_deref(), only, *format, out).await
        }
        ConfigCommand::Set {
            key, value, system, ..
        } => set::set(&mut talk, &tidy(key), value, *system).await,
        ConfigCommand::Unset { key, system } => set::unset(&mut talk, &tidy(key), *system).await,
        ConfigCommand::Edit { system, project } => {
            edit::edit(&mut talk, check::Only::of(*system, *project), console).await
        }
        ConfigCommand::Trust { yes, no } => {
            let answer = match (yes, no) {
                (true, _) => Some(true),
                (_, true) => Some(false),
                _ => None,
            };
            trust::trust(&mut talk, answer, console, out).await
        }
    }
}

/// 核心的回应。
pub(crate) enum Reply {
    /// 接受了：`result`。
    Done(Value),
    /// 拒绝了：`error`。
    Refused(Value),
}

/// 连核心以前就知道不对的（施工 8-3）：`set --project`（项目配置只能手改）、不在终端里的 `edit`。说一句，交回退出码 2。
fn early(
    command: &ConfigCommand,
    language: Language,
    console: &dyn Console,
    err: &mut dyn Write,
) -> Option<u8> {
    let said = match command {
        ConfigCommand::Set { project: true, .. } => language.project_by_hand(),
        ConfigCommand::Edit { .. } if !console.terminal() => language.edit_needs_terminal(),
        _ => return None,
    };
    say(err, said);
    Some(MISUSE)
}

/// 问核心时手里的几样。
pub(crate) struct Talk<'a> {
    rpc: &'a mut Rpc,
    plan: &'a ConfigPlan,
    err: &'a mut dyn Write,
}

impl Talk<'_> {
    /// 发一条请求，交回 `result`。被拒绝的照 [`Talk::refused`] 说一句；核心断开的说一句。都交回退出码。
    pub(crate) async fn ask(&mut self, method: &str, params: Value) -> Result<Value, u8> {
        match self.request(method, params).await? {
            Reply::Done(result) => Ok(result),
            Reply::Refused(error) => Err(self.refused(&error)),
        }
    }

    /// 发一条请求，交回回应：接受了的、被拒绝的（原因由调用的一方看，施工 8-3：`edit`、`trust` 的冲突另说一句）。核心断开的、
    /// 写不出去的说一句，交回退出码。
    pub(crate) async fn request(&mut self, method: &str, params: Value) -> Result<Reply, u8> {
        let language = self.plan.language;
        match self.rpc.call(method, params).await {
            Ok(Some(reply)) => match reply.get("error") {
                None => Ok(Reply::Done(reply["result"].clone())),
                Some(error) => Ok(Reply::Refused(error.clone())),
            },
            Ok(None) => {
                say(self.err, &language.disconnected());
                Err(exit::ERROR)
            }
            Err(error) => {
                say(self.err, &error.to_string());
                Err(exit::ERROR)
            }
        }
    }

    /// 被拒绝了：写了 `data.problems` 的一条一句（不认识的键带最近的键名），没写的说核心的原话。交回退出码。
    pub(crate) fn refused(&mut self, error: &Value) -> u8 {
        let problems = error["data"]["problems"].as_array();
        match problems.filter(|problems| !problems.is_empty()) {
            Some(problems) => {
                for problem in problems {
                    say(self.err, problem["message"].as_str().unwrap_or_default());
                }
            }
            None => say(self.err, error["message"].as_str().unwrap_or_default()),
        }
        exit::ERROR
    }

    /// 工作目录，照协议的写法。
    fn cwd(&self) -> String {
        self.plan.cwd.to_string_lossy().into_owned()
    }
}

/// `get [键…]`：只写一个键的只印值，字不带引号；别的一行一个 `键 = 值`，照键名排。`--format json`：`items` 原样。
async fn get(talk: &mut Talk<'_>, keys: &[String], format: Format, out: &mut dyn Write) -> u8 {
    let mut params = json!({"cwd": talk.cwd()});
    if !keys.is_empty() {
        params["keys"] = json!(keys);
    }
    let result = match talk.ask("config.get", params).await {
        Ok(result) => result,
        Err(code) => return code,
    };
    match format {
        Format::Json => say(out, &result["items"].to_string()),
        Format::Text => write(out, &render::values(&result["items"], keys.len() == 1)),
    }
    exit::OK
}

/// `explain <键>`：`config.get`（带目录、每一层）再 `config.schema`，第一行名字、说明、什么时候生效，下面每一层一行。
/// `--format json`：那一项原样，多 `name`、`description`。
async fn explain(talk: &mut Talk<'_>, key: &str, format: Format, out: &mut dyn Write) -> u8 {
    let params = json!({"keys": [key], "cwd": talk.cwd(), "all": true});
    let got = match talk.ask("config.get", params).await {
        Ok(result) => result,
        Err(code) => return code,
    };
    let schema = match talk.ask("config.schema", json!({"keys": [key]})).await {
        Ok(result) => result,
        Err(code) => return code,
    };
    let item = &got["items"][key];
    let said = &schema["items"][0];
    match format {
        Format::Json => {
            let mut item = item.clone();
            item["name"] = said["name"].clone();
            item["description"] = said["description"].clone();
            say(out, &item.to_string());
        }
        Format::Text => {
            let places = paths::Places::of(talk.plan);
            for line in render::explain(key, item, said, talk.plan.language, &places) {
                write(out, &line.paint(talk.plan.color));
            }
        }
    }
    exit::OK
}

/// `path`：这一层的文件在哪，绝对路径，一行，文件还没有也印。`--project` 没找到的印仓库的根下的
/// `.miyu/config.toml`，标准错误上说一句还没有这个文件。
async fn path(talk: &mut Talk<'_>, system: bool, project: bool, out: &mut dyn Write) -> u8 {
    let params = match project {
        true => json!({"cwd": talk.cwd()}),
        false => json!({}),
    };
    let result = match talk.ask("config.get", params).await {
        Ok(result) => result,
        Err(code) => return code,
    };
    let places = paths::Places::of(talk.plan);
    let layer = match (system, project) {
        (true, _) => "system",
        (_, true) => "project",
        _ => "personal",
    };
    match result["files"][layer]["file"].as_str() {
        Some(file) => say(out, &places.absolute(file).to_string_lossy()),
        None => {
            say(out, &paths::planned(&talk.plan.cwd).to_string_lossy());
            say(talk.err, talk.plan.language.no_file_yet());
        }
    }
    exit::OK
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

/// 人敲的键照核心的写法规整（施工 8-3 补，`cli/config.md`「怎么走」第 8 条）：照 `miyu_config::key` 拆开再拼回去，多余的
/// 引号去掉、点两边的空格去掉，名字里有点的那一段照旧带引号。核心回应里的键是规整过的，拿人敲的原样去找会找不到
/// （`set` 写进去了却说没改）。拆不开的原样交，由核心照原来的话报错。
fn tidy(key: &str) -> String {
    miyu_config::key::split(key).map_or_else(
        || key.to_string(),
        |segments| miyu_config::key::join(&segments),
    )
}

//! `miyu config`（`docs/blueprint/cli/config.md`，`config.md`「命令行」第十条，施工 8-2）：看配置的四个子命令，
//! `get`、`check`、`explain`、`path`。它们只是协议的客户端（`22-命令行.md` O5）：连上核心，`config.get`、
//! `config.schema`、`config.check`，照回应印。改、写、信任的几个随 8-3。
//!
//! 连核心照 `miyu recap`：核心在跑的照样连；没在跑、又没设 `DEEPSEEK_API_KEY` 的不拉起，说没有可用的模型，退出码 5
//! （8-6 以后 key 来自配置，改成一律拉起）。握手以后给人看的字照回应的 `language`（施工 8-2）。
//!
//! 退出码：0 成了（`check` 没有错误）；1 核心拒绝了、`check` 有错误、连不上核心；2 参数不对（在主程序里）；5 同上。

mod check;
mod paths;
mod render;
#[cfg(test)]
mod tests;

use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use miyu_ipc::{ConnectError, Connection, connect_or_start};
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
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return failed(&error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return failed(&error.to_string());
    }
    // 照 `miyu recap`：8-6 以前核心只在起来时读 key，没设 key、核心又没在跑的不拉起。
    let key = std::env::var("DEEPSEEK_API_KEY").is_ok_and(|key| !key.trim().is_empty());
    let connected = match key {
        true => connect_or_start(&root, start)
            .await
            .map_err(|error| error.to_string()),
        false => match miyu_ipc::connect(&root).await {
            Ok(connected) => Ok(connected),
            Err(ConnectError::NotRunning) => {
                say(&mut io::stderr(), &language.no_model());
                return exit::NO_MODEL;
            }
            Err(error) => Err(error.to_string()),
        },
    };
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
    };
    config_on(
        connection,
        &token,
        &plan,
        &mut io::stdout(),
        &mut io::stderr(),
    )
    .await
}

/// 在一条连上了的连接上办一次 `miyu config`：握手，照子命令问核心、印，交回退出码。测试照它在进程里走一遍。
pub async fn config_on(
    connection: Connection,
    token: &str,
    plan: &ConfigPlan,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
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
        ConfigCommand::Explain { key, format } => explain(&mut talk, key, *format, out).await,
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
    }
}

/// 问核心时手里的几样。
pub(crate) struct Talk<'a> {
    rpc: &'a mut Rpc,
    plan: &'a ConfigPlan,
    err: &'a mut dyn Write,
}

impl Talk<'_> {
    /// 发一条请求，交回 `result`。被拒绝的：写了 `data.problems` 的一条一句（不认识的键带最近的键名），没写的说核心的
    /// 原话；核心断开的说一句。都交回退出码。
    pub(crate) async fn ask(&mut self, method: &str, params: Value) -> Result<Value, u8> {
        let language = self.plan.language;
        match self.rpc.call(method, params).await {
            Ok(Some(reply)) => match reply.get("error") {
                None => Ok(reply["result"].clone()),
                Some(error) => {
                    let problems = error["data"]["problems"].as_array();
                    match problems.filter(|problems| !problems.is_empty()) {
                        Some(problems) => {
                            for problem in problems {
                                say(self.err, problem["message"].as_str().unwrap_or_default());
                            }
                        }
                        None => say(self.err, error["message"].as_str().unwrap_or_default()),
                    }
                    Err(exit::ERROR)
                }
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

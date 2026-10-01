//! `miyu login`、`miyu logout`（`docs/blueprint/cli/login.md`，`config.md`「怎么走」第十一条，施工 8-5）：管密钥的命令，
//! 照 opencode 的 `auth login`、`auth list`、`auth logout` 和 codex 的 `login`、`logout`（2026-10-01 项目主人定）。它们只是
//! 协议的客户端：`secret.list`、`secret.set`、`secret.delete`。
//!
//! - `miyu login [名字]`：不写名字的在终端里从配置里用到的、设过的里面选，也可以敲一个新名字；贴 key 不回显，管道进来的
//!   整份读。key 从不写在命令行上（会进 shell 的历史），也从不印出来，前几位也不印。
//! - `miyu login --list [--format text|json]`：哪几个设了、谁在用，不给看 key。
//! - `miyu logout [名字]`：删掉一个；不写名字的在终端里从设过的里面选。
//!
//! 名字不合写法、不写名字又不在终端里：连核心以前就说，退出码 2。连核心照 `miyu config`（8-6 以前没设 `DEEPSEEK_API_KEY`、
//! 核心又没在跑的不拉起，退出码 5）。要问人的经 [`Console`]：测试换成照剧本回的。
//!
//! 退出码：0 成了；1 核心拒绝了、没收到 key、没选、要删的没设过、连不上核心；2 参数不对；5 同上。

mod pick;

use std::io::{self, IsTerminal, Write};
use std::process::{Command, ExitCode};

use clap::Args;
use serde_json::{Value, json};

use miyu_config::secret::valid_name;
use miyu_ipc::{ConnectError, Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::ask::Format;
use crate::config::Console;
use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::{self, Line, say, write};

/// 参数不对（`cli/main.md`「参数写错时」）。
const MISUSE: u8 = 2;

/// `miyu login` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Login {
    /// 存哪一个；不写的在终端里选。
    #[arg(value_name = "NAME", conflicts_with = "list")]
    pub name: Option<String>,
    /// 只列出哪几个设了。
    #[arg(long)]
    pub list: bool,
    /// `--list` 的输出格式。
    #[arg(long, value_enum, default_value = "text", requires = "list")]
    pub format: Format,
}

/// `miyu logout` 的参数。
#[derive(Debug, Clone, Args)]
pub struct Logout {
    /// 删哪一个；不写的在终端里从设过的里面选。
    #[arg(value_name = "NAME")]
    pub name: Option<String>,
}

/// 这一次做什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyCommand {
    /// 存一个 key：名字写了的直接贴，没写的先选。
    Login(Option<String>),
    /// 列出来。
    List(Format),
    /// 删一个。
    Logout(Option<String>),
}

impl From<Login> for KeyCommand {
    fn from(login: Login) -> KeyCommand {
        match login.list {
            true => KeyCommand::List(login.format),
            false => KeyCommand::Login(login.name),
        }
    }
}

impl From<Logout> for KeyCommand {
    fn from(logout: Logout) -> KeyCommand {
        KeyCommand::Logout(logout.name)
    }
}

/// 这一次做什么、怎么印。
#[derive(Debug, Clone)]
pub struct LoginPlan {
    /// 做什么。
    pub command: KeyCommand,
    /// 界面语言：握手以后换成回应的。
    pub language: Language,
    /// 标准输出上不上色（`--list` 里没设的那几行是灰的）。
    pub color: bool,
    /// 标准错误上的灰字上不上色。
    pub gray: bool,
}

/// 跑一次 `miyu login` 或 `miyu logout`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn login(command: KeyCommand, start: impl FnOnce() -> Command) -> ExitCode {
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
    ExitCode::from(runtime.block_on(run(command, start)))
}

/// 在运行时里：先查参数，找数据根、连上核心，照命令办。
async fn run(command: KeyCommand, start: impl FnOnce() -> Command) -> u8 {
    let language = language::current();
    let mut console = crate::config::Terminal::current();
    if let Some(code) = early(&command, language, &console, &mut io::stderr()) {
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
    // 照 `miyu config`：8-6 以前核心只在起来时读 key，没设 key、核心又没在跑的不拉起。
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
    let no_color = std::env::var_os("NO_COLOR");
    let plan = LoginPlan {
        command,
        language,
        color: shown::colored(io::stdout().is_terminal(), no_color.as_deref()),
        gray: shown::colored(io::stderr().is_terminal(), no_color.as_deref()),
    };
    login_on(
        connection,
        &token,
        &plan,
        &mut console,
        &mut io::stdout(),
        &mut io::stderr(),
    )
    .await
}

/// 在一条连上了的连接上办一次：握手，照命令问核心、问人、印，交回退出码。测试照它在进程里走一遍。
pub async fn login_on(
    connection: Connection,
    token: &str,
    plan: &LoginPlan,
    console: &mut dyn Console,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    if let Some(code) = early(&plan.command, plan.language, console, err) {
        return code;
    }
    let mut rpc = Rpc::new(connection, "login");
    let plan = match link::hello(&mut rpc, token, &plan.language, false, err).await {
        Ok(hello) => LoginPlan {
            language: link::spoken(&hello, plan.language),
            ..plan.clone()
        },
        Err(code) => return code,
    };
    let mut keys = Keys {
        rpc: &mut rpc,
        plan: &plan,
        console,
        err,
    };
    match &plan.command {
        KeyCommand::List(format) => keys.list(*format, out).await,
        KeyCommand::Login(name) => keys.login(name.as_deref()).await,
        KeyCommand::Logout(name) => keys.logout(name.as_deref()).await,
    }
}

/// 连核心以前就知道不对的：名字不合写法、不写名字又不在终端里。说一句，交回退出码 2。
fn early(
    command: &KeyCommand,
    language: Language,
    console: &dyn Console,
    err: &mut dyn Write,
) -> Option<u8> {
    let (name, which) = match command {
        KeyCommand::Login(name) => (name, "login"),
        KeyCommand::Logout(name) => (name, "logout"),
        KeyCommand::List(_) => return None,
    };
    match name {
        Some(name) if !valid_name(name) => say(err, &language.bad_key_name(name)),
        None if !console.terminal() => say(err, &language.pick_needs_terminal(which)),
        _ => return None,
    }
    Some(MISUSE)
}

/// 问核心、问人时手里的几样。
struct Keys<'a> {
    rpc: &'a mut Rpc,
    plan: &'a LoginPlan,
    console: &'a mut dyn Console,
    err: &'a mut dyn Write,
}

impl Keys<'_> {
    /// 发一条请求，交回 `result`；被拒绝的交回原因码和核心的原话；核心断开的说一句，交回退出码。
    async fn request(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<Result<Value, (String, String)>, u8> {
        match self.rpc.call(method, params).await {
            Ok(Some(reply)) => Ok(match reply.get("error") {
                None => Ok(reply["result"].clone()),
                Some(error) => Err((
                    error["data"]["reason"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    error["message"].as_str().unwrap_or_default().to_string(),
                )),
            }),
            Ok(None) => {
                say(self.err, &self.plan.language.disconnected());
                Err(exit::ERROR)
            }
            Err(error) => {
                say(self.err, &error.to_string());
                Err(exit::ERROR)
            }
        }
    }

    /// `secret.list` 的每一个；被拒绝的说核心的原话，交回退出码。
    async fn listed(&mut self) -> Result<Vec<Value>, u8> {
        match self.request("secret.list", json!({})).await? {
            Ok(result) => Ok(result["secrets"].as_array().cloned().unwrap_or_default()),
            Err((_, message)) => {
                say(self.err, &message);
                Err(exit::ERROR)
            }
        }
    }

    /// 印一行灰字。
    fn gray(&mut self, text: String) {
        write(self.err, &Line::gray(text).paint(self.plan.gray));
    }

    /// `login --list`：设了的在前，配置里用到、还没设的在后（灰字）；`--format json` 印回应原样。
    async fn list(&mut self, format: Format, out: &mut dyn Write) -> u8 {
        let secrets = match self.listed().await {
            Ok(secrets) => secrets,
            Err(code) => return code,
        };
        match format {
            Format::Json => say(out, &json!({ "secrets": secrets }).to_string()),
            Format::Text if secrets.is_empty() => say(out, self.plan.language.no_keys_yet()),
            Format::Text => {
                for line in pick::table(&secrets, self.plan.language) {
                    write(out, &line.paint(self.plan.color));
                }
            }
        }
        exit::OK
    }

    /// `login`：名字（没写的先选）、读 key、`secret.set`。
    async fn login(&mut self, name: Option<&str>) -> u8 {
        let language = self.plan.language;
        let name = match name {
            Some(name) => name.to_string(),
            None => match self.pick(false).await {
                Ok(name) => name,
                Err(code) => return code,
            },
        };
        let read = match self.console.typed() {
            true => {
                write(self.err, &language.paste_key(&name));
                self.console.hidden().map(Option::unwrap_or_default)
            }
            false => self.console.all(),
        };
        let key = match read {
            Ok(key) if !key.trim().is_empty() => key,
            Ok(_) => {
                say(self.err, language.no_key_given());
                return exit::ERROR;
            }
            Err(error) => {
                say(self.err, &error.to_string());
                return exit::ERROR;
            }
        };
        let params = json!({"name": name, "value": key});
        match self.request("secret.set", params).await {
            Ok(Ok(result)) => {
                let replaced = result["replaced"] == json!(true);
                self.gray(language.key_saved(&name, replaced));
                exit::OK
            }
            Ok(Err((_, message))) => {
                say(self.err, &message);
                exit::ERROR
            }
            Err(code) => code,
        }
    }

    /// `logout`：名字（没写的从设过的里面选）、`secret.delete`。
    async fn logout(&mut self, name: Option<&str>) -> u8 {
        let language = self.plan.language;
        let name = match name {
            Some(name) => name.to_string(),
            None => match self.pick(true).await {
                Ok(name) => name,
                Err(code) => return code,
            },
        };
        match self.request("secret.delete", json!({"name": name})).await {
            Ok(Ok(_)) => {
                self.gray(language.key_deleted(&name));
                exit::OK
            }
            Ok(Err((reason, _))) if reason == "unknown_secret" => {
                say(self.err, &language.no_such_key(&name));
                exit::ERROR
            }
            Ok(Err((_, message))) => {
                say(self.err, &message);
                exit::ERROR
            }
            Err(code) => code,
        }
    }

    /// 在终端里选一个（第十一条第 3 条）：`logout` 只列设过的。编号表、问的话都在标准错误上，标准输出留给 `--list`。
    async fn pick(&mut self, logout: bool) -> Result<String, u8> {
        let language = self.plan.language;
        let mut secrets = self.listed().await?;
        if logout {
            secrets.retain(|secret| secret["set"] == json!(true));
        }
        if secrets.is_empty() {
            return Err(match logout {
                true => {
                    say(self.err, language.no_keys_yet());
                    exit::ERROR
                }
                false => {
                    say(self.err, language.no_keys_used());
                    MISUSE
                }
            });
        }
        say(self.err, language.keys_heading(logout));
        for line in pick::numbered(&secrets, language) {
            write(self.err, &line.paint(self.plan.gray));
        }
        write(self.err, language.pick_key(logout));
        let typed = self.console.line().ok().flatten().unwrap_or_default();
        match pick::chosen(typed.trim(), &secrets) {
            pick::Chosen::Name(name) => Ok(name),
            pick::Chosen::Nothing => {
                say(self.err, language.not_picked());
                Err(exit::ERROR)
            }
            pick::Chosen::Bad(typed) => {
                say(self.err, &language.bad_key_name(&typed));
                Err(MISUSE)
            }
        }
    }
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

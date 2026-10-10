//! `miyu memory`（`docs/blueprint/cli/memory.md`，施工 R-3 再补）：在 shell 里看她记了你什么，搜、记、改、忘、清空，现在就
//! 整理（`dream`，施工 R-7 补）。连上
//! 核心（照 `miyu rename`：没在跑就拉起来），照子命令发 `memory.*`（`protocol.md`），照回应印一句或者一行一条。
//!
//! 做成了退出码 0；核心拒的照核心的原话印在标准错误上，退出码 1；参数写错的 2，由 clap 管。

mod shown;

use std::io::{self, Write};
use std::process::{Command, ExitCode};

use clap::{Args, Subcommand};
use serde_json::{Map, Value, json};

use miyu_ipc::{Connection, connect_or_start};
use miyu_kernel::time::UtcOffset;
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::ask::Format;
use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::say;

/// 出厂认识的四类（`memory.md` 的记忆日志）：`add --class` 只收它们，`list --class` 照它们挑。
const CLASSES: [&str; 4] = ["user", "feedback", "episode", "reference"];

/// `miyu memory` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。选项写在子命令前后都行。
#[derive(Debug, Clone, Args)]
pub struct Memory {
    /// 哪个子命令；不写的是 `list`。
    #[command(subcommand)]
    pub command: Option<MemoryCommand>,
    /// 哪个人格的记忆；不写照核心的默认人格。
    #[arg(long, global = true, value_name = "ID", conflicts_with = "session")]
    pub persona: Option<String>,
    /// 这个会话用的那一份记忆。
    #[arg(short = 's', long, global = true)]
    pub session: Option<String>,
    /// `list`、`search`：连作废的一起。
    #[arg(long, global = true)]
    pub forgotten: bool,
    /// `list`：只要这一类；`add`：记成这一类，不写是 `user`。
    #[arg(long, global = true, value_parser = CLASSES)]
    pub class: Option<String>,
    /// `forget`：为什么忘。
    #[arg(long, global = true, value_name = "REASON")]
    pub why: Option<String>,
    /// `list`、`search` 输出的格式。
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    pub format: Format,
}

/// `miyu memory` 的子命令。
#[derive(Debug, Clone, Subcommand)]
pub enum MemoryCommand {
    /// 列出来，新的在前。
    List,
    /// 搜：几个词用一个空格连起来。
    Search {
        /// 搜什么。
        #[arg(required = true, num_args = 1..)]
        words: Vec<String>,
    },
    /// 记一条。
    Add {
        /// 记什么。
        #[arg(required = true, num_args = 1..)]
        words: Vec<String>,
    },
    /// 改一条：编号，新的话。
    Edit {
        /// 改哪一条，`m<序号>`。
        id: String,
        /// 改成什么。
        #[arg(required = true, num_args = 1..)]
        words: Vec<String>,
    },
    /// 忘掉一条。
    Forget {
        /// 忘哪一条，`m<序号>`。
        id: String,
    },
    /// 清空。
    Clear {
        /// 清哪一种。
        #[command(subcommand)]
        what: Clear,
    },
    /// 现在就整理：合掉重复的、改掉过时的、更新摘要（`memory.md` 第七条第 9 款）。
    Dream,
}

/// 清空的两种（`memory.md` 第二条第 5 款，一般知识那一层随 O 线）。
#[derive(Debug, Clone, Subcommand)]
pub enum Clear {
    /// 清掉这个会话记下的；不写会话的是上一次 `miyu ask` 开的那个。
    Session {
        /// 哪个会话；不写的照 `-s`，再没有的是上一次 `miyu ask` 开的那个。名字另起：和全局的 `--session` 同名的话，
        /// clap 会把写在这里的值也填进 `--session`。
        #[arg(id = "cleared", value_name = "SESSION")]
        session: Option<String>,
    },
    /// 清掉她关于你的全部记忆。
    Me,
}

/// 这一次做什么，照哪种语言说，日期照哪个时区。
#[derive(Debug, Clone)]
pub struct MemoryPlan {
    /// 读好的参数。
    pub args: Memory,
    /// 界面语言。
    pub language: Language,
    /// 这台机器此刻的时区：`list`、`search` 照它把时刻换成那一天。
    pub offset: UtcOffset,
}

/// 跑一次 `miyu memory`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn memory(args: Memory, start: impl FnOnce() -> Command) -> ExitCode {
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
async fn run(args: Memory, start: impl FnOnce() -> Command) -> u8 {
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
    let plan = MemoryPlan {
        args,
        language: language::current(),
        offset: offset(),
    };
    memory_on(
        connection,
        &token,
        &plan,
        &mut io::stdout(),
        &mut io::stderr(),
    )
    .await
}

/// 在一条连上了的连接上做一次：握手、照子命令发、印，交回退出码；印的写在 `out`，出错的写在 `err`。测试照它在进程里走一遍。
pub async fn memory_on(
    connection: Connection,
    token: &str,
    plan: &MemoryPlan,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut rpc = Rpc::new(connection, "memory");
    let language = &plan.language;
    if let Err(code) = link::hello(&mut rpc, token, language, false, err).await {
        return code;
    }
    match act(&mut rpc, plan, out, err).await {
        Ok(()) => exit::OK,
        Err(code) => code,
    }
}

/// 照子命令发一条 `memory.*`，照回应印。
async fn act(
    rpc: &mut Rpc,
    plan: &MemoryPlan,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let (args, language) = (&plan.args, &plan.language);
    let command = args.command.clone().unwrap_or(MemoryCommand::List);
    let mut params = room(args);
    let (method, extra) = match &command {
        MemoryCommand::List => (
            "memory.list",
            json!({"forgotten": args.forgotten, "class": args.class}),
        ),
        MemoryCommand::Search { words } => (
            "memory.search",
            json!({"query": words.join(" "), "forgotten": args.forgotten}),
        ),
        MemoryCommand::Add { words } => (
            "memory.remember",
            json!({"class": args.class.as_deref().unwrap_or("user"), "text": words.join(" ")}),
        ),
        MemoryCommand::Edit { id, words } => {
            ("memory.update", json!({"id": id, "text": words.join(" ")}))
        }
        MemoryCommand::Forget { id } => ("memory.forget", json!({"id": id, "why": args.why})),
        MemoryCommand::Clear { what: Clear::Me } => ("memory.forget", json!({"clear": "me"})),
        MemoryCommand::Clear {
            what: Clear::Session { session },
        } => {
            // 清会话的：那个会话也是找哪一间的那个（`protocol.md` 的 `memory.forget`）。
            let session = match session.as_ref().or(args.session.as_ref()) {
                Some(session) => session.clone(),
                None => link::latest_oneshot(rpc, language, err).await?,
            };
            params = json!({"session": session});
            ("memory.forget", json!({"clear": "session"}))
        }
        MemoryCommand::Dream => ("memory.dream", json!({})),
    };
    merge(&mut params, extra);
    let result = link::request_saying(rpc, method, params, language, err, |error| {
        refused(error, language)
    })
    .await?;
    let said = match command {
        MemoryCommand::List | MemoryCommand::Search { .. } => {
            return print(&result, plan, &command, out);
        }
        MemoryCommand::Add { .. } => language.remembered(id_of(&result)),
        MemoryCommand::Edit { .. } => language.changed(id_of(&result)),
        MemoryCommand::Forget { .. } => return Ok(()),
        MemoryCommand::Clear { .. } => language.cleared(result["cleared"].as_u64().unwrap_or(0)),
        MemoryCommand::Dream => {
            let count = |field: &str| result[field].as_u64().unwrap_or(0);
            language.dreamed(
                count("given"),
                count("revised"),
                count("retired"),
                result["summary"].as_bool().unwrap_or(false),
            )
        }
    };
    say(out, &said);
    Ok(())
}

/// `list`、`search` 的回应印出来：`json` 的原样印那一串；`text` 的一行一条，一条都没有的说一句。
fn print(
    result: &Value,
    plan: &MemoryPlan,
    command: &MemoryCommand,
    out: &mut dyn Write,
) -> Result<(), u8> {
    let memories = result["memories"].as_array().cloned().unwrap_or_default();
    if plan.args.format == Format::Json {
        say(out, &Value::Array(memories).to_string());
        return Ok(());
    }
    if memories.is_empty() {
        let none = match command {
            MemoryCommand::Search { .. } => plan.language.nothing_found(),
            _ => plan.language.no_memories(),
        };
        say(out, none);
        return Ok(());
    }
    for row in shown::listed(&memories, plan.offset, &plan.language) {
        say(out, &row);
    }
    Ok(())
}

/// 被拒绝时说的那一句：太长的照 `data` 说这一条几个字、一条最多几个（核心的原话是给程序看的，说的是去 `data` 里看）；别的
/// 照核心的原话。
fn refused(error: &Value, language: &Language) -> String {
    let data = &error["data"];
    match (
        data["reason"].as_str(),
        data["chars"].as_u64(),
        data["limit"].as_u64(),
    ) {
        (Some("memory_too_long"), Some(chars), Some(limit)) => language.too_long(chars, limit),
        _ => language.refused(error["message"].as_str().unwrap_or_default()),
    }
}

/// 找哪一间：写了 `--persona` 的带 `persona`，写了 `-s` 的带 `session`，都不写的都不带（核心照默认人格）。
fn room(args: &Memory) -> Value {
    let mut params = Map::new();
    if let Some(persona) = &args.persona {
        params.insert("persona".to_string(), json!(persona));
    }
    if let Some(session) = &args.session {
        params.insert("session".to_string(), json!(session));
    }
    Value::Object(params)
}

/// 把 `extra` 里写了的几格并进 `params`；`null` 的不写（`class`、`why` 没给的）。
fn merge(params: &mut Value, extra: Value) {
    if let (Value::Object(params), Value::Object(extra)) = (params, extra) {
        params.extend(extra.into_iter().filter(|(_, value)| !value.is_null()));
    }
}

fn id_of(result: &Value) -> &str {
    result["id"].as_str().unwrap_or_default()
}

/// 这台机器此刻的时区（照核心给会话的环境的算法，`miyu-endpoint` 的 `sessions.rs`）。
fn offset() -> UtcOffset {
    let minutes = jiff::Zoned::now().offset().seconds() / 60;
    UtcOffset::from_minutes(minutes).unwrap_or(UtcOffset::UTC)
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

#[cfg(test)]
mod tests;

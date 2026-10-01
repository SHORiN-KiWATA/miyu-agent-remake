//! `miyu rename`（`docs/blueprint/cli/rename.md`，施工 3-8 五补）：连上核心（照 `miyu undo` 的规矩：没设 key 的不拉起），找
//! 当前会话（最新的那个一次性会话；`--session` 指定别的），发 `session.set_meta` 给它起名。
//!
//! 起好了什么都不印，退出码 0（照 `mv`）；被拒绝（标题空的、太长的、没有这个会话……）照核心的原话印在标准错误上，退出码 1；
//! 核心没在跑、又没设 key 的 5。

use std::io::{self, Write};
use std::process::{Command, ExitCode};

use clap::Args;
use serde_json::json;

use miyu_ipc::{Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::say;

/// `miyu rename` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Rename {
    /// 新的标题：几个词用一个空格连起来，至少一个。
    #[arg(required = true, num_args = 1..)]
    pub words: Vec<String>,
    /// 哪个会话；不写的是上一次 `miyu ask` 开的那个。
    #[arg(short = 's', long)]
    pub session: Option<String>,
}

/// 这一次给哪个会话起什么名、出错的话照哪种语言说。
#[derive(Debug, Clone)]
pub struct RenamePlan {
    /// 哪个会话；没有的是最新的那个一次性会话。
    pub session: Option<String>,
    /// 新的标题，原样交给核心：去掉前后空白、量长短是核心的事（`protocol.md` 的 `session.set_meta`）。
    pub title: String,
    /// 界面语言。
    pub language: Language,
}

/// 跑一次 `miyu rename`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn rename(args: Rename, start: impl FnOnce() -> Command) -> ExitCode {
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

/// 在运行时里：找数据根、连上核心、起名。
async fn run(args: Rename, start: impl FnOnce() -> Command) -> u8 {
    let language = language::current();
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
    let plan = RenamePlan {
        session: args.session,
        title: args.words.join(" "),
        language,
    };
    rename_on(connection, &token, &plan, &mut io::stderr()).await
}

/// 在一条连上了的连接上起一次名：握手、找会话、发 `session.set_meta`，交回退出码；出错的话写在 `err` 上。测试照它在进程里
/// 走一遍。
pub async fn rename_on(
    connection: Connection,
    token: &str,
    plan: &RenamePlan,
    err: &mut dyn Write,
) -> u8 {
    let mut rpc = Rpc::new(connection, "rename");
    let language = &plan.language;
    if let Err(code) = link::hello(&mut rpc, token, language, false, err).await {
        return code;
    }
    let session = match &plan.session {
        Some(session) => session.clone(),
        None => match link::latest_oneshot(&mut rpc, language, err).await {
            Ok(session) => session,
            Err(code) => return code,
        },
    };
    let params = json!({"session": session, "title": plan.title});
    match link::request(&mut rpc, "session.set_meta", params, language, err).await {
        Ok(_) => exit::OK,
        Err(code) => code,
    }
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

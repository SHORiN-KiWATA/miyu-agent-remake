//! `miyu recap`（`docs/blueprint/cli/recap.md`，施工 3-8 四补）：连上核心（回顾要请求模型，照 `miyu ask` 的规矩拉起），找
//! 当前会话（和 `miyu undo` 一样，最新的那个一次性会话；`--session` 指定别的），发 `session.recap`，把那一句印在标准输出上。
//!
//! 不订阅、不跟回合：回顾不开回合，回应里就是那一句。印出来的只有那一句，好接进管道；给人看的字只有出错的那几句，照核心、
//! `miyu undo` 的原话。写好了退出码 0；被拒绝（没有能回顾的、回顾没写成、没有这个会话……）1；没有可用的模型 5。

use std::io::{self, Write};
use std::process::{Command, ExitCode};

use clap::Args;
use serde_json::json;

use miyu_ipc::{ConnectError, Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::say;

/// `miyu recap` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Recap {
    /// 哪个会话；不写的是上一次 `miyu ask` 开的那个。
    #[arg(short = 's', long)]
    pub session: Option<String>,
}

/// 这一次回顾哪个会话、出错的话照哪种语言说。
#[derive(Debug, Clone)]
pub struct RecapPlan {
    /// 哪个会话；没有的是最新的那个一次性会话。
    pub session: Option<String>,
    /// 界面语言。
    pub language: Language,
}

/// 跑一次 `miyu recap`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn recap(args: Recap, start: impl FnOnce() -> Command) -> ExitCode {
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

/// 在运行时里：找数据根、连上核心、要一句回顾。
async fn run(args: Recap, start: impl FnOnce() -> Command) -> u8 {
    let language = language::current();
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return failed(&error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return failed(&error.to_string());
    }
    // 照 `miyu compact`：回顾要请求模型。没设 key 的，核心在跑的照样连（它可能有），没在跑的不拉起。
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
    let plan = RecapPlan {
        session: args.session,
        language,
    };
    recap_on(
        connection,
        &token,
        &plan,
        &mut io::stdout(),
        &mut io::stderr(),
    )
    .await
}

/// 在一条连上了的连接上要一句回顾：握手、找会话、发 `session.recap`，把那一句印在 `out` 上，交回退出码。测试照它在进程里
/// 走一遍。
pub async fn recap_on(
    connection: Connection,
    token: &str,
    plan: &RecapPlan,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let mut rpc = Rpc::new(connection, "recap");
    // 握手以后照核心回的语言说（施工 8-2）。
    let plan = &match link::hello(&mut rpc, token, &plan.language, false, err).await {
        Ok(hello) => RecapPlan {
            language: link::spoken(&hello, plan.language),
            ..plan.clone()
        },
        Err(code) => return code,
    };
    let language = &plan.language;
    let session = match &plan.session {
        Some(session) => session.clone(),
        None => match link::latest_oneshot(&mut rpc, language, err).await {
            Ok(session) => session,
            Err(code) => return code,
        },
    };
    let params = json!({"session": session});
    match link::request(&mut rpc, "session.recap", params, language, err).await {
        Ok(result) => {
            say(out, result["text"].as_str().unwrap_or_default());
            exit::OK
        }
        Err(code) => code,
    }
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

//! `miyu compact`（`docs/blueprint/cli/compact.md`，施工 6-8，命令名 2026-09-29 项目主人定）：连上核心（压缩要请求
//! 模型，照 `miyu ask` 的规矩拉起），找当前会话（和 `miyu undo` 一样，最新的那个一次性会话；`--session` 指定别的），
//! 发 `session.compact`，跟着单开的那一轮，照 `miyu ask` 压缩那一行、用量那一行的样子印，全在标准错误上。
//!
//! 压好了退出码 0；被拒绝、压缩失败 1；被打断 3；没有可用的模型 5。

use std::io::{self, IsTerminal};
use std::process::{Command, ExitCode};

use clap::Args;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use miyu_ipc::{ConnectError, Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::human::Human;
use miyu_store::root::DataRoot;

use crate::ask::follow::Follow;
use crate::ask::{Format, Plan, Screen, Target, Watching, exit, follow_turn, presses};
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::{self, say};

/// `miyu compact` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Compact {
    /// 给摘要的要求：几个词用空格连起来，可以不写。
    #[arg(num_args = 0..)]
    pub words: Vec<String>,
    /// 哪个会话；不写的是上一次 `miyu ask` 开的那个。
    #[arg(short = 's', long)]
    pub session: Option<String>,
}

/// 这一次压哪个会话、附什么要求、怎么印。
#[derive(Debug, Clone)]
pub struct CompactPlan {
    /// 哪个会话；没有的是最新的那个一次性会话。
    pub session: Option<String>,
    /// 人附的要求；没附的没有。
    pub instructions: Option<String>,
    /// 界面语言。
    pub language: Language,
}

impl Compact {
    /// 要求：写的几个词用一个空格连起来；一个都没写的没有（只有空白的由内核当没写）。
    pub fn instructions(&self) -> Option<String> {
        (!self.words.is_empty()).then(|| self.words.join(" "))
    }
}

/// 跑一次 `miyu compact`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn compact(args: Compact, start: impl FnOnce() -> Command) -> ExitCode {
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

/// 在运行时里：找数据根、连上核心、压。
async fn run(args: Compact, start: impl FnOnce() -> Command) -> u8 {
    let language = language::current();
    let env = Env::current();
    let root = match DataRoot::locate(&env) {
        Ok(root) => root,
        Err(error) => return failed(&error.to_string()),
    };
    if let Err(error) = root.prepare() {
        return failed(&error.to_string());
    }
    // 照 `miyu ask`：压缩要请求模型。没设 key 的，核心在跑的照样连（它可能有），没在跑的不拉起。
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
    let plan = CompactPlan {
        instructions: args.instructions(),
        session: args.session,
        language,
    };
    let mut out = io::stdout();
    let mut err = io::stderr();
    let live = err.is_terminal();
    let gray = shown::colored(live, std::env::var_os("NO_COLOR").as_deref());
    let mut screen = Screen {
        out: &mut out,
        err: &mut err,
        gray,
        live,
    };
    compact_on(connection, &token, &plan, &mut screen, presses()).await
}

/// 在一条连上了的连接上压一次：握手、找会话、订阅、发 `session.compact`，跟着单开的那一轮照 `miyu ask` 的样子印，交回
/// 退出码。`presses` 是一次次的 Ctrl+C：第一次打断这一轮，别的头在压缩期间排着的话接着发（`queued: "send"`：多半是
/// 另一个终端里的 `miyu ask -c`，退回了那边就一直等不到自己那一轮）；第二次不等了。测试照它在进程里走一遍。
pub async fn compact_on(
    connection: Connection,
    token: &str,
    plan: &CompactPlan,
    screen: &mut Screen<'_>,
    presses: mpsc::Receiver<()>,
) -> u8 {
    let mut rpc = Rpc::new(connection, "compact");
    let language = &plan.language;
    if let Err(code) = link::hello(&mut rpc, token, language, false, screen.err).await {
        return code;
    }
    let session = match &plan.session {
        Some(session) => session.clone(),
        None => match link::latest_oneshot(&mut rpc, language, screen.err).await {
            Ok(session) => session,
            Err(code) => return code,
        },
    };
    let subscribe = json!({"session": session, "stream": "events"});
    let subscribed = link::request(
        &mut rpc,
        "subscribe",
        subscribe.clone(),
        language,
        screen.err,
    )
    .await;
    if let Err(code) = subscribed {
        return code;
    }
    let sent = match rpc.send("session.compact", request(&session, plan)).await {
        Ok(sent) => sent,
        Err(error) => {
            say(screen.err, &error.to_string());
            return exit::ERROR;
        }
    };
    let printing = printing(plan.language);
    let mut follow = Follow::new(&session, &sent, &printing);
    let watching = Watching {
        subscribe: &subscribe,
        session: &session,
        queued: "send",
        language,
    };
    follow_turn(&mut rpc, &mut follow, &watching, screen, presses).await
}

/// `session.compact` 的参数：没附要求的不写这一格。
fn request(session: &str, plan: &CompactPlan) -> Value {
    match &plan.instructions {
        Some(instructions) => json!({"session": session, "instructions": instructions}),
        None => json!({"session": session}),
    }
}

/// 跟着那一轮印的时候照的：给人看，界面语言照 `language`。那一轮只压缩、不调工具，工作目录、给人看的每一步的字用
/// 不上。
fn printing(language: Language) -> Plan {
    Plan {
        text: String::new(),
        target: Target::Continue,
        format: Format::Text,
        cwd: String::new(),
        dirs: Vec::new(),
        language,
        human: Human::default(),
        home: None,
    }
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

#[cfg(test)]
mod tests;

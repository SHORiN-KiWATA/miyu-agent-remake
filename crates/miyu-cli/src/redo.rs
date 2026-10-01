//! `miyu redo`（`docs/blueprint/cli/redo.md`，施工 4-7 再补；2026-09-30 项目主人定：核心一个命令，只重做最后一轮）：连上核心
//! （重做要请求模型，照 `miyu ask` 的规矩拉起），找当前会话（和 `miyu undo` 一样，最新的那个一次性会话；`--session` 指定
//! 别的），订阅，发 `session.redo`（写了话的换掉开那一轮的那一句），先照 `miyu undo` 印撤掉了哪一轮，再照 `miyu ask`
//! 跟着新的一轮边收边打。
//!
//! 退出码照 `miyu ask`：照常结束 0；被拒绝（无法重做……）、出错 1；被打断 3；有几步要确认没做 4；没有可用的模型 5。

use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use clap::Args;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use miyu_ipc::{Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::human::Human;
use miyu_store::root::DataRoot;

use crate::ask::follow::Follow;
use crate::ask::{Format, Plan, Screen, Target, Watching, exit, follow_turn, human, presses};
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::{self, say};

/// `miyu redo` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
pub struct Redo {
    /// 换成的话：几个词用空格连起来，可以不写。
    #[arg(num_args = 0..)]
    pub words: Vec<String>,
    /// 哪个会话；不写的是上一次 `miyu ask` 开的那个。
    #[arg(short = 's', long)]
    pub session: Option<String>,
}

impl Redo {
    /// 换成的话：写的几个词用一个空格连起来；一个都没写的没有，原样重做。
    pub fn text(&self) -> Option<String> {
        (!self.words.is_empty()).then(|| self.words.join(" "))
    }
}

/// 这一次重做哪个会话、换成什么话、怎么印。
#[derive(Debug, Clone)]
pub struct RedoPlan {
    /// 哪个会话；没有的是最新的那个一次性会话。
    pub session: Option<String>,
    /// 换成的话；没有的原样重做。
    pub text: Option<String>,
    /// 界面语言。
    pub language: Language,
    /// 给人看的字，照界面语言读的那一份：新的一轮每一步怎么写。
    pub human: Human,
    /// 家目录：路径写成 `~/…`。
    pub home: Option<PathBuf>,
}

/// 跑一次 `miyu redo`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn redo(args: Redo, start: impl FnOnce() -> Command) -> ExitCode {
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

/// 在运行时里：找数据根、连上核心、重做。
async fn run(args: Redo, start: impl FnOnce() -> Command) -> u8 {
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
    let plan = RedoPlan {
        text: args.text(),
        session: args.session,
        human: human(&env, &language),
        home: env.home.clone(),
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
    redo_on(connection, &token, &plan, &mut screen, presses()).await
}

/// 在一条连上了的连接上重做一次：握手、找会话、订阅、发 `session.redo`，回应到了照 `miyu undo` 印撤掉了哪一轮，再照
/// `miyu ask` 跟着新的一轮印，交回退出码。`presses` 是一次次的 Ctrl+C：第一次打断这一轮（排着的退回，照 `miyu ask`），
/// 第二次不等了。测试照它在进程里走一遍。
pub async fn redo_on(
    connection: Connection,
    token: &str,
    plan: &RedoPlan,
    screen: &mut Screen<'_>,
    presses: mpsc::Receiver<()>,
) -> u8 {
    let mut rpc = Rpc::new(connection, "redo");
    let hello = match link::hello(&mut rpc, token, &plan.language, false, screen.err).await {
        Ok(hello) => hello,
        Err(code) => return code,
    };
    let unsandboxed = link::unsandboxed(&hello);
    // 握手以后照核心回的语言说（施工 8-2）：换了的，给人看的字也照新的那种读一份。
    let spoken;
    let plan = match link::spoken(&hello, plan.language) {
        language if language == plan.language => plan,
        language => {
            spoken = RedoPlan {
                language,
                human: human(&miyu_store::env::Env::current(), &language),
                ..plan.clone()
            };
            &spoken
        }
    };
    let language = &plan.language;
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
    let sent = match rpc.send("session.redo", request(&session, plan)).await {
        Ok(sent) => sent,
        Err(error) => {
            say(screen.err, &error.to_string());
            return exit::ERROR;
        }
    };
    let printing = printing(plan);
    let mut follow = Follow::new(&session, &sent, &printing);
    follow.redoing();
    // 握手的回应说沙盒用不了：新的一轮执行命令都要确认，这里确认不了，照 `miyu ask` 最先说一句。
    if let Some(reason) = &unsandboxed {
        follow.unsandboxed(reason, screen);
    }
    let watching = Watching {
        subscribe: &subscribe,
        session: &session,
        queued: "return",
        language,
        deadline: None,
    };
    follow_turn(&mut rpc, &mut follow, &watching, screen, presses).await
}

/// `session.redo` 的参数：没写话的不写 `text` 这一格。
fn request(session: &str, plan: &RedoPlan) -> Value {
    match &plan.text {
        Some(text) => json!({"session": session, "text": text}),
        None => json!({"session": session}),
    }
}

/// 跟着新的一轮印的时候照的：给人看。会话在哪个目录里干活等回应来了照它换上（`Follow` 收重做的回应时），这里先空着。
fn printing(plan: &RedoPlan) -> Plan {
    Plan {
        text: String::new(),
        target: Target::Continue,
        format: Format::Text,
        cwd: String::new(),
        dirs: Vec::new(),
        files: Vec::new(),
        language: plan.language,
        human: plan.human.clone(),
        home: plan.home.clone(),
        timeout: None,
        from: None,
    }
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

#[cfg(test)]
mod tests;

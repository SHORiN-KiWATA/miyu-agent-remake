//! `miyu setup`（`docs/blueprint/cli/setup.md`，`models.md`「怎么走」第七条第 5 条，施工 8-11）：第一次接入模型。找现成的
//! key 和本机的模型服务，没有的从目录里搜一家、贴 key，试通了，选主对话的模型，写进系统配置。一行行问、敲数字、贴 key
//! 不回显，参数能跳过对应的一步（2026-10-01 项目主人看过、定了）。全屏的引导做在各个头里，照同一组方法。
//!
//! 它只是协议的客户端：`provider.detect`、`provider.catalog`、`provider.test`、`secret.set`、`config.set`，`miyu ask` 先问
//! `config.get` 的 `models.chat`（[`model_ready_on`]）。要问人的经 [`Console`]：测试换成照剧本回的。一步步怎么走在 `setup/flow.rs`，
//! 选一家在 `setup/choose.rs`，选模型在 `setup/model.rs`，编号表在 `setup/pick.rs`。
//!
//! 退出码：0 写好了；1 没选、没收到 key、试不通（不在终端里，或者没有上一步可回）、核心拒绝了、连不上；2 参数不对。

mod choose;
mod custom;
mod flow;
mod model;
mod pick;

use std::fmt;
use std::io::{self, IsTerminal, Write};
use std::process::{Command, ExitCode};
use std::sync::Arc;

use clap::Args;

use miyu_ipc::{Connection, connect_or_start};
use miyu_store::env::Env;
use miyu_store::root::DataRoot;

use crate::config::Console;
use crate::exit;
use crate::language::{self, Language};
use crate::link;
use crate::rpc::Rpc;
use crate::shown::{self, say};
use flow::Flow;

/// 参数不对（`cli/main.md`「参数写错时」）。
const MISUSE: u8 = 2;

/// `miyu setup` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Default, Args)]
pub struct Setup {
    /// 跳过选一家：目录、档案里的这一家。
    #[arg(long, value_name = "ID")]
    pub provider: Option<String>,
    /// 跳过贴 key：照核心环境里的这个变量取。
    #[arg(long, value_name = "VAR")]
    pub env: Option<String>,
    /// 跳过选模型：这一家的模型名。
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,
}

/// 头这边的环境里哪些变量设了（`cli/setup.md`「怎么走」第 3 条第 1 款）：和核心的比，说清核心看不到哪几个。只问有没有，
/// 不拿值。
#[derive(Clone)]
pub struct HeadEnv(Arc<dyn Fn(&str) -> bool + Send + Sync>);

impl HeadEnv {
    /// 这个进程的环境：设了、去掉前后空白不是空的。
    pub fn process() -> HeadEnv {
        HeadEnv(Arc::new(|name| {
            std::env::var(name).is_ok_and(|value| !value.trim().is_empty())
        }))
    }

    /// 只有这几个设了（测试用）。
    pub fn of(names: &[&str]) -> HeadEnv {
        let names: Vec<String> = names.iter().map(|name| (*name).to_string()).collect();
        HeadEnv(Arc::new(move |name| names.iter().any(|set| set == name)))
    }

    /// 变量 `name` 设了。
    fn has(&self, name: &str) -> bool {
        (self.0)(name)
    }
}

impl fmt::Debug for HeadEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HeadEnv(…)")
    }
}

/// 这一次照什么走、怎么印。
#[derive(Debug, Clone)]
pub struct SetupPlan {
    /// 参数。
    pub setup: Setup,
    /// 界面语言：握手以后换成回应的。
    pub language: Language,
    /// 标准错误上的灰字上不上色。
    pub gray: bool,
    /// 头这边的环境。
    pub here: HeadEnv,
}

/// 跑一次 `miyu setup`，交回退出码。`start` 给出拉起核心的命令：主程序自己加上 `core`。
pub fn setup(args: Setup, start: impl FnOnce() -> Command) -> ExitCode {
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

/// 在运行时里：先查参数，找数据根、连上核心，走一遍。
async fn run(args: Setup, start: impl FnOnce() -> Command) -> u8 {
    let mut console = crate::config::Terminal::current();
    let plan = SetupPlan {
        setup: args,
        language: language::current(),
        gray: shown::colored(
            io::stderr().is_terminal(),
            std::env::var_os("NO_COLOR").as_deref(),
        ),
        here: HeadEnv::process(),
    };
    if let Some(code) = early(&plan, &console, &mut io::stderr()) {
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
    setup_on(connection, &token, &plan, &mut console, &mut io::stderr()).await
}

/// 在一条连上了的连接上走一遍：握手，照先后问核心、问人，交回退出码。测试照它在进程里走一遍。问人、印的都在 `err` 上。
pub async fn setup_on(
    connection: Connection,
    token: &str,
    plan: &SetupPlan,
    console: &mut dyn Console,
    err: &mut dyn Write,
) -> u8 {
    if let Some(code) = early(plan, console, err) {
        return code;
    }
    let mut rpc = Rpc::new(connection, "setup");
    let plan = match link::hello(&mut rpc, token, &plan.language, false, err).await {
        Ok(hello) => SetupPlan {
            language: link::spoken(&hello, plan.language),
            ..plan.clone()
        },
        Err(code) => return code,
    };
    Flow::new(&mut rpc, &plan, console, err).run().await
}

/// `miyu ask` 说话之前（`cli/ask.md` 第 2 条，`models.md` 第七条第 6 条）：握手，问 `config.get` 的 `models.chat`（不带
/// `cwd`，不算项目配置，和 `model.list` 的 `uses.chat` 是同一个值；`model.list` 会顺手在后台拉供应商的列表，「施工时定的」
/// 8-11），有值的就有模型。没有的：在终端里先走一遍 setup（照 `plan`，参数都不写），不在终端里说没有模型那一句，退出码 5。
///
/// # Errors
///
/// 没走完 setup、不在终端里、核心拒绝了、断开了：交回退出码。
pub async fn model_ready_on(
    connection: Connection,
    token: &str,
    plan: &SetupPlan,
    console: &mut dyn Console,
    err: &mut dyn Write,
) -> Result<(), u8> {
    let mut rpc = Rpc::new(connection, "ask");
    let hello = link::hello(&mut rpc, token, &plan.language, false, err).await?;
    let language = link::spoken(&hello, plan.language);
    let params = serde_json::json!({"keys": ["models.chat"]});
    let got = link::request(&mut rpc, "config.get", params, &language, err).await?;
    if !got["items"]["models.chat"]["value"].is_null() {
        return Ok(());
    }
    if !console.terminal() {
        say(err, &language.no_model());
        return Err(exit::NO_MODEL);
    }
    say(err, language.setup_first());
    let plan = SetupPlan {
        setup: Setup::default(),
        language,
        ..plan.clone()
    };
    match Flow::new(&mut rpc, &plan, console, err).run().await {
        exit::OK => Ok(()),
        code => Err(code),
    }
}

/// 连核心以前就知道不对的：不在终端里又没写 `--provider`。说一句，交回退出码 2。
fn early(plan: &SetupPlan, console: &dyn Console, err: &mut dyn Write) -> Option<u8> {
    if plan.setup.provider.is_none() && !console.terminal() {
        say(err, plan.language.setup_needs_terminal());
        return Some(MISUSE);
    }
    None
}

/// 连上核心之前就出错了：原因写在标准错误上。
fn failed(reason: &str) -> u8 {
    say(&mut io::stderr(), reason);
    exit::ERROR
}

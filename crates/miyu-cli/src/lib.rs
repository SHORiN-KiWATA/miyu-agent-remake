//! 命令行的头（`docs/designs/22-命令行.md`，施工 3-9 下）：`miyu ask`，最薄的头，也是协议的参考实现；`miyu undo`（`miyu rewind`）、
//! `miyu restore`（施工 4-7 下，改名施工 4-7 补）。
//!
//! - [`Ask`]：`miyu ask` 的参数；[`ask()`]：跑一次，交回退出码；
//! - [`Undo`]：`miyu undo`、`miyu restore` 的参数；[`undo()`]：撤一次（恢复一次），交回退出码；[`undo_on`]：在连上了的
//!   连接上撤一次，测试照它在进程里走一遍；
//! - [`talk`]：在一条连上了的连接上把一句话说完：握手、找会话、订阅、发、跟着那一轮边收边打。测试照它在
//!   进程里走一遍；
//! - [`Redo`]：`miyu redo` 的参数；[`redo()`]：重做一次，交回退出码；[`redo_on`]：在连上了的连接上重做一次，测试照它在
//!   进程里走一遍（施工 4-7 再补）；
//! - [`Compact`]：`miyu compact` 的参数；[`compact()`]：压一次，交回退出码；[`compact_on`]：在连上了的连接上压一次，
//!   测试照它在进程里走一遍（施工 6-8）；
//! - [`Recap`]：`miyu recap` 的参数；[`recap()`]：要一句回顾，印出来，交回退出码；[`recap_on`]：在连上了的连接上要一次，
//!   测试照它在进程里走一遍（施工 3-8 四补）；
//! - [`Rename`]：`miyu rename` 的参数；[`rename()`]：给会话起名，交回退出码；[`rename_on`]：在连上了的连接上起一次名，
//!   测试照它在进程里走一遍（施工 3-8 五补）；
//! - [`Sandbox`]：`miyu sandbox setup`、`remove` 的参数；[`sandbox()`]：Windows 上装好、撤掉沙盒用户，交回退出码
//!   （施工 5-8）；
//! - [`Config`]：`miyu config` 的参数；[`config()`]：看配置一次，交回退出码；[`config_on`]：在连上了的连接上办一次，
//!   测试照它在进程里走一遍（施工 8-2）；
//! - [`Login`]、[`Logout`]：`miyu login`、`miyu logout` 的参数；[`login()`]：存、列、删一次 key，交回退出码；[`login_on`]：
//!   在连上了的连接上办一次，测试照它在进程里走一遍（施工 8-5）；
//! - [`Setup`]：`miyu setup` 的参数；[`setup()`]：第一次接入模型，交回退出码；[`setup_on`]：在连上了的连接上走一遍；
//!   [`model_ready_on`]：`miyu ask` 说话之前看有没有模型、没有就先走一遍（施工 8-11）；
//! - [`language`]：给人看的话跟着界面语言。

mod ask;
mod compact;
mod config;
pub mod head;
pub mod help;
pub mod language;
mod link;
mod login;
mod misuse;
pub mod packages;
mod recap;
mod redo;
mod rename;
mod rpc;
mod sandbox;
mod setup;
mod shown;
mod undo;
mod web;

pub use ask::{Ask, Format, Plan, Screen, Target, ask, exit, talk};
pub use compact::{Compact, CompactPlan, compact, compact_on};
pub use config::{
    Check, Config, ConfigCommand, ConfigPlan, Console, Terminal, check, config, config_on,
};
pub use login::{KeyCommand, Login, LoginPlan, Logout, login, login_on};
pub use misuse::misuse;
pub use recap::{Recap, RecapPlan, recap, recap_on};
pub use redo::{Redo, RedoPlan, redo, redo_on};
pub use rename::{Rename, RenamePlan, rename, rename_on};
pub use sandbox::{Action as SandboxAction, OwnerArgs, Sandbox, sandbox};
pub use setup::{HeadEnv, Setup, SetupPlan, model_ready_on, setup, setup_on};
pub use undo::{Direction, Undo, UndoPlan, undo, undo_on};
pub use web::{Web, web, web_on};

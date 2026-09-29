//! 命令行的头（`docs/designs/22-命令行.md`，施工 3-9 下）：`miyu ask`，最薄的头，也是协议的参考实现；`miyu undo`（`miyu rewind`）、
//! `miyu restore`（施工 4-7 下，改名施工 4-7 补）。
//!
//! - [`Ask`]：`miyu ask` 的参数；[`ask()`]：跑一次，交回退出码；
//! - [`Undo`]：`miyu undo`、`miyu restore` 的参数；[`undo()`]：撤一次（恢复一次），交回退出码；[`undo_on`]：在连上了的
//!   连接上撤一次，测试照它在进程里走一遍；
//! - [`talk`]：在一条连上了的连接上把一句话说完：握手、找会话、订阅、发、跟着那一轮边收边打。测试照它在
//!   进程里走一遍；
//! - [`Compact`]：`miyu compact` 的参数；[`compact()`]：压一次，交回退出码；[`compact_on`]：在连上了的连接上压一次，
//!   测试照它在进程里走一遍（施工 6-8）；
//! - [`Sandbox`]：`miyu sandbox setup`、`remove` 的参数；[`sandbox()`]：Windows 上装好、撤掉沙盒用户，交回退出码
//!   （施工 5-8）；
//! - [`language`]：给人看的话跟着界面语言。

mod ask;
mod compact;
pub mod help;
pub mod language;
mod link;
mod misuse;
mod rpc;
mod sandbox;
mod shown;
mod undo;

pub use ask::{Ask, Format, Plan, Screen, Target, ask, exit, talk};
pub use compact::{Compact, CompactPlan, compact, compact_on};
pub use misuse::misuse;
pub use sandbox::{Action as SandboxAction, OwnerArgs, Sandbox, sandbox};
pub use undo::{Direction, Undo, UndoPlan, undo, undo_on};

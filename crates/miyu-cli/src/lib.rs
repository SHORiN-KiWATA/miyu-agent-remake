//! 命令行的头（`docs/designs/22-命令行.md`，施工 3-9 下）：`miyu ask`，最薄的头，也是协议的参考实现。
//!
//! - [`Ask`]：`miyu ask` 的参数；[`ask()`]：跑一次，交回退出码；
//! - [`talk`]：在一条连上了的连接上把一句话说完：握手、找会话、订阅、发、跟着那一轮边收边打。测试照它在
//!   进程里走一遍；
//! - [`language`]：给人看的话跟着界面语言。

mod ask;
pub mod language;

pub use ask::{Ask, Format, Plan, Screen, Target, ask, exit, localize, talk};

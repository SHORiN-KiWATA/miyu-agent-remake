//! 这个 crate 的集成测试并成一个程序（施工 0-3 三补，`docs/construction/0-3-三平台CI（三补）.md`）：一个文件一个模块，
//! `support` 只编一次，只链接一次。自己装日志订阅者的几个照旧各是各的程序（调用点记下谁在听是全进程的，同一个进程里
//! 别的测试同时碰到会漏听；全局的一个进程只能装一次），
//! 在 `Cargo.toml` 里另列。新加的测试文件在下面添一行。

mod support;

mod agents;
mod ask;
mod attach;
mod compact;
mod config;
mod from;
mod login;
mod memory;
mod recap;
mod redo;
mod rename;
mod setup;
mod setup_custom;
mod setup_skip;
mod steps;
mod undo;

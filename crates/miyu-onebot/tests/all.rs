//! 这个 crate 的集成测试并成一个程序（施工 0-3 三补，`docs/construction/0-3-三平台CI（三补）.md`）：一个文件一个模块，
//! `support` 只编一次，只链接一次。自己装日志订阅者的几个照旧各是各的程序（调用点记下谁在听是全进程的，同一个进程里
//! 别的测试同时碰到会漏听；全局的一个进程只能装一次），
//! 在 `Cargo.toml` 里另列。新加的测试文件在下面添一行。

mod support;

mod apply;
mod backstage;
mod budget;
mod called;
mod called_limits;
mod calls;
mod commands;
mod control;
mod core;
mod dependencies;
mod disciplines;
mod followed;
mod frames;
mod group;
mod ids;
mod judged;
mod judged_persona;
mod linking;
mod listen;
mod logs;
mod media;
mod members;
mod no_token;
mod outbound;
mod page;
mod pipe;
mod platform;
mod private;
mod pronoun;
mod provider;
mod queue;
mod queue_waits;
mod reactions;
mod receipts;
mod reload;
mod replies;
mod rules;
mod segments;
mod settings;
mod skipped;
mod spawned;
mod status_file;
mod stdio;
mod superseded;
mod text;
mod texts;
mod tuning;
mod undelivered;
mod venue;
mod web;
mod whitelist;

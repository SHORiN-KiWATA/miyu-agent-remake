//! Miyu 的内核。
//!
//! 内核只提供机制，不决定策略：回合循环、上下文投影、工具调度、权限检查点。
//! 它是纯逻辑，不做任何 I/O：不发网络请求，不读写文件，不读时钟。
//! 它只做一件事：接收输入，更新状态，输出要做的事（动作）。
//! 真正的 I/O 由执行器完成，结果再作为新的输入送回来。
//!
//! 设计见 `docs/designs/02-内核.md`。现在有的是事件和它的零件（`03-事件模型.md`）：
//!
//! - [`id`]、[`time`]：编号、名字和时间；
//! - [`origin`]：事件的 `by`，由谁引起；
//! - [`block`]：内容块，消息和工具结果都由它组成；
//! - [`event`]：事件本身，和日志里一行 JSON 之间的转换；
//! - [`history`]：有效历史，投影要用的那一段，从最近一次压缩算起、去掉撤销的回合；
//! - [`ledger`]：日志的账本，追加一条事件之前照规矩查一遍；
//! - [`request`]：统一的请求，投影交给驱动的那一份，和它的规范字节、哈希、指纹；
//! - [`assemble`]：组装请求的接口，给一段有效历史，出一份统一的请求；
//! - [`facts`]：环境和状态的事实，写成什么、该不该注入；
//! - [`session`]：会话的状态机，送进一条输入，出来一串动作；
//! - [`accumulate`]：流式累积器，模型输出的增量拼成完整的内容块；
//! - [`raw`]：原样的 JSON，驱动私有数据和不认识的种类都用它；
//! - [`template`]：模板与转义，给模型看的字怎么拼，不可信的字段怎么转。

pub mod accumulate;
pub mod assemble;
pub mod block;
pub mod event;
pub mod facts;
mod format_error;
pub mod history;
pub mod id;
pub mod ledger;
pub mod origin;
pub mod raw;
pub mod request;
pub mod session;
pub mod template;
mod text_enum;
pub mod time;

#[cfg(test)]
mod test_support;

pub use format_error::FormatError;

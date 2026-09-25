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
//! - [`raw`]：原样的 JSON，驱动私有数据和不认识的种类都用它。

pub mod block;
pub mod event;
mod format_error;
pub mod id;
pub mod origin;
pub mod raw;
mod text_enum;
pub mod time;

#[cfg(test)]
mod test_support;

pub use format_error::FormatError;

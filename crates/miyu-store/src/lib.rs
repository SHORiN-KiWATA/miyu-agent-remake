//! Miyu 的存储执行器（`docs/designs/07-存储.md`）：第 3 层，做真的 I/O。
//!
//! 内核只把要追加的事件、要存的内容写成动作交出来（`02-内核.md` 第四节「执行器怎么回动作」），
//! 这里把它们真的写到磁盘上。现在有的：
//!
//! - [`mod@env`]：找数据根要看的几样，从进程里读一次；
//! - [`root`]：数据根和缓存目录在哪，第一次用时建好骨架；
//! - [`log`]：会话日志，按段存成 JSONL，一批一次写入、一次同步，打开时自检；
//! - [`blob`]：大内容按内容哈希存，先写临时文件、同步、再改名，读的时候核对哈希。

pub mod blob;
mod durable;
pub mod env;
pub mod log;
pub mod root;

#[cfg(test)]
mod test_support;

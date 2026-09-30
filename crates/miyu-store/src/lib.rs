//! Miyu 的存储执行器（`docs/designs/07-存储.md`）：第 3 层，做真的 I/O。
//!
//! 内核只把要追加的事件、要存的内容写成动作交出来（`02-内核.md` 第四节「执行器怎么回动作」），
//! 这里把它们真的写到磁盘上。现在有的：
//!
//! - [`mod@env`]：找数据根要看的几样，从进程里读一次；系统的语言（施工 8-1）；
//! - [`root`]：数据根和缓存目录在哪，第一次用时建好骨架；
//! - [`log`]：会话日志，按段存成 JSONL，一批一次写入、一次同步，打开时自检；
//! - [`blob`]：大内容按内容哈希存，先写临时文件、同步、再改名，读的时候核对哈希；
//! - [`generated`]：核心生成的派生文件，一样的不写，不一样的先写临时文件再替换（施工 8-1）；
//! - [`config_file`]：读、写配置文件，写的时候顺着链接、先写临时文件再替换、替换前再读一次（施工 8-2、8-3）；
//! - [`journal`]：系统日志、账号日志，配置的改动、项目配置的信任一行一条（施工 8-3）；
//! - [`resources`]：资源目录在哪，读出一个人格要用的原文，交给 `miyu-policy` 拼快照；
//! - [`human`]：资源目录里给人看的字，照说法换成一句话（施工 4-5 上）；
//! - [`jobs`]：会话目录下后台命令的输出（施工 7-3）；
//! - [`trash`]：回收处，删掉的会话挪进来、满了时限再真删（施工 3-8 三补）；
//! - [`index`]：会话列表的索引，SQLite，派生的，随时可以删掉照日志重建（施工 3-8 七补）。

pub mod blob;
pub mod config_file;
mod durable;
pub mod env;
pub mod generated;
pub mod human;
pub mod index;
pub mod jobs;
pub mod journal;
pub mod log;
pub mod resources;
pub mod root;
pub mod trash;

#[cfg(test)]
mod test_support;

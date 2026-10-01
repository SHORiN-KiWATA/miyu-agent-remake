//! 配置清单（`docs/blueprint/config.md`，`docs/designs/14-配置.md` G1、G9、G10，施工 8-1）：第 2 层的纯逻辑，不碰磁盘，
//! 进来的是字，出去的是字。
//!
//! 每个配置项只在清单里声明一次：模块在自己的 crate 里用 [`settings!`] 写一个设置类型，宏生成这几项的清单
//! （[`Item`]）和从最终值（[`Values`]）变过来的设置类型。核心把各模块的清单登记成一张表，照它：
//!
//! - [`list::check`]：查清单写得对不对（键不重复、不互为前缀、合写法，默认值过自己的校验）；
//! - [`words::check`]：查资源里给人看的字和清单对不对得上；
//! - [`schema::render`]、[`reference::render`]：生成两份 JSON Schema 和参考文件；
//! - [`parse::parse`]：读一份配置的字，照清单认，记下每一项在第几行，写错的报 [`problem::Problem`]（施工 8-2）；
//! - [`merge::merge`]：分层合出最终值和来源，项目配置只认收紧的（施工 8-2）；
//! - [`problem::tell`]：一条问题照一种语言说成话（施工 8-2）；
//! - [`edit::apply`]：在一份配置的字上改一项、删一项，只动那一项，别的字节一个不变（施工 8-3）；
//! - [`key`]：键里人起的名字那一段（`providers.<id>.base_url`）：真的键怎么拆、怎么接、对不对得上清单里的样子（施工 8-6）；
//! - [`secret`]：密钥的名字、配置里引用密钥的写法、密钥文件的字怎么读、怎么改一行，取出来的密钥 [`secret::Secret`]
//!   不会被印出来（施工 8-5）。
//!
//! 给人看的字（名字、说明、几句话）住在资源目录里，由读资源的那一层照 [`Words`] 交进来。
//!
//! 现在有选项、开关、密钥、整数、网址、名字、引用、列表（[`Kind`]）：照「不为以后写代码」，别的类型哪一步用到哪一步加。

pub mod edit;
mod item;
pub mod key;
pub mod list;
pub mod merge;
pub mod parse;
pub mod problem;
pub mod reference;
pub mod schema;
pub mod secret;
mod value;
pub mod words;

#[cfg(test)]
mod named_tests;
#[cfg(test)]
mod test_support;

pub use item::{Applies, Control, Item, Kind, Layer, Tighten, Ui, duration};
pub use value::{Number, Setting, Value, Values};
pub use words::{ConfigWords, ItemWords, Missing, Words};

//! 配置清单（`docs/blueprint/config.md`，`docs/designs/14-配置.md` G1、G10，施工 8-1）：第 2 层的纯逻辑，不碰磁盘，
//! 进来的是字，出去的是字。
//!
//! 每个配置项只在清单里声明一次：模块在自己的 crate 里用 [`settings!`] 写一个设置类型，宏生成这几项的清单
//! （[`Item`]）和从最终值（[`Values`]）变过来的设置类型。核心把各模块的清单登记成一张表，照它：
//!
//! - [`list::check`]：查清单写得对不对（键不重复、不互为前缀、合写法，默认值过自己的校验）；
//! - [`words::check`]：查资源里给人看的字和清单对不对得上；
//! - [`schema::render`]、[`reference::render`]：生成两份 JSON Schema 和参考文件。
//!
//! 给人看的字（名字、说明、几句话）住在资源目录里，由读资源的那一层照 [`Words`] 交进来。
//!
//! 现在只有选项一种类型（[`Kind`]）：照「不为以后写代码」，别的类型哪一步用到哪一步加。

mod item;
pub mod list;
pub mod reference;
pub mod schema;
mod value;
pub mod words;

#[cfg(test)]
mod test_support;

pub use item::{Applies, Control, Item, Kind, Layer, Ui};
pub use value::{Value, Values};
pub use words::{ConfigWords, ItemWords, Missing, Words};

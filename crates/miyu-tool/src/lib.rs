//! 工具的接口和工具目录（`docs/designs/05-内核接口.md` 第六节「工具的规格」、第八节「启用、注册与
//! 目录快照」，施工 4-1）。
//!
//! 工具是内核之外的软件（`10-自带软件.md` B1）：每件工具报出自己的规格，核心起来时登记进工具目录，
//! 登记完就冻结。造会话时，会话照目录把工具面存进策略快照（`03-事件模型.md` E5），以后一直照快照发。
//!
//! - [`Spec`]：一件工具的规格，第一批四格；
//! - [`Tool`]：一件工具：报规格，报一次调用要碰的路径（[`Target`]，施工 4-3 下），执行一次调用（[`Call`] 进、
//!   [`Done`] 出，施工 4-2）；
//! - [`Catalog`]：工具目录，登记时查重名、名字和参数格式的写法。

mod catalog;
mod run;
#[cfg(feature = "testkit")]
pub mod testkit;

pub use catalog::{Catalog, CatalogError, Problem};
pub use run::{Call, Done, Progress, Running, Target};

use miyu_kernel::raw::RawJson;
use miyu_kernel::tool::Access;

/// 一件工具的规格（05 第六节）：第一批四格（施工 4-1）。显示名、摘要随施工 4-5；默认超时、场所、
/// 组，用得上时再加。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    /// 工具名：英文，稳定不变，模型照它调。只用英文字母、数字、`_`、`-`，1 到 64 个字符。
    pub name: String,
    /// 给模型看的说明，英文，原样进 tools 数组。
    pub description: String,
    /// 参数的 JSON Schema，原样进 tools 数组，一个字节不改：必须是 `{"type":"object",…}`。
    pub parameters: RawJson,
    /// 访问类别：权限策略、能不能和别的一起跑，都看它。
    pub access: Access,
}

/// 一件工具。
pub trait Tool: Send + Sync {
    /// 它的规格。
    fn spec(&self) -> &Spec;

    /// 这次调用要碰的路径、是读是写：照参数算，不碰磁盘（施工 4-3 下）。换成真实的位置、查边界是执行前的
    /// 链的事。默认一条都没有，例如执行命令。参数不对的，也交回空的：执行时再报错。
    fn targets(&self, _call: &Call) -> Vec<Target> {
        Vec::new()
    }

    /// 执行一次调用：执行器在它自己的任务里跑交回的 future，执行中的输出交给 `progress`。叫停就是
    /// 丢掉这个 future（施工 4-2）。
    fn run(&self, call: Call, progress: Progress) -> Running<'_>;
}

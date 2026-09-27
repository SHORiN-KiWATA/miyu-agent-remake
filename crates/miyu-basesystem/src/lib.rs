//! 基础系统（`docs/designs/10-自带软件.md` 第三节）：随发行附带、几乎每个预设都打开的几件工具，软件包的编号
//! 是 `basesystem`。
//!
//! 工具的说明、参数格式、输出里给她看的几句都放在资源目录的 `software/basesystem/` 下（`26-提示词.md`
//! 第八节），核心起来时读。现在 [`tools`] 里有读的三件：`read`（施工 4-4 上）、`glob`、`grep`（施工 4-4 下），
//! 和写的 `write`（施工 4-6 上）。
//! 名字、参数、输出照成熟 harness 的规范，以 Claude Code 为主（`10-自带软件.md` 第十节）。

mod blocking;
mod common;
mod glob;
mod grep;
mod load;
mod pattern;
mod read;
mod replace;
mod text;
mod walk;
mod write;

use std::path::Path;
use std::sync::Arc;

use miyu_tool::Tool;

pub use load::LoadError;

/// 基础系统的每件工具：照资源目录 `resources` 里的字造好。
///
/// # Errors
///
/// 哪一份字读不出来、写法不对，说是哪一份。
pub fn tools(resources: &Path) -> Result<Vec<Arc<dyn Tool>>, LoadError> {
    let common = common::Common::load(resources)?;
    Ok(vec![
        Arc::new(read::Read::load(resources, common.clone())?),
        Arc::new(glob::Glob::load(resources, common.clone())?),
        Arc::new(grep::Grep::load(resources, common.clone())?),
        Arc::new(write::Write::load(resources, common)?),
    ])
}

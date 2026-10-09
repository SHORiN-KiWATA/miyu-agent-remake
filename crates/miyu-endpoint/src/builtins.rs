//! 编进核心的内置包的工具（施工 F-5 中，设计 `30-插件框架.md` 第九节）：照清单登记哪几件在 `miyu-core`（只有它知道编进来了
//! 哪些、工具的字在哪），装卸以后端点经这个端口重新要，当场换进工具目录。没装这个端口的核心（测试里造的）装卸时不动工具。

use std::sync::Arc;

use miyu_store::packages::Found;
use miyu_tool::Tool;

/// 一个内置包的工具：包的编号和它的几件。
pub type Group = (String, Vec<Arc<dyn Tool>>);

/// 内置包的工具从哪来。
pub trait Builtins: Send + Sync {
    /// 照清单 `found` 交回装了的内置包的工具：包的编号和它的几件。没装的不在里面。
    ///
    /// # Errors
    ///
    /// 工具的字读不出来、写法不对：一句英文。
    fn tools(&self, found: &[Found]) -> Result<Vec<Group>, String>;
}

//! 别的 harness 发来的话的标签装进快照（施工 7-10，`docs/blueprint/kernel/request.md`「别的 harness 发来的话」）：
//! `resources/core/harness/` 下的两份原文。以前造的快照里没有，读成没有：那种话照人的话原样渲染。

use std::collections::BTreeMap;

use miyu_assemble::HarnessTexts as Rendered;
use miyu_kernel::template::Template;
use serde::{Deserialize, Serialize};

use crate::snapshot::BuildError;

/// 别的 harness 发来的话的标签（`harness/` 下，文件名是下划线换成 `-` 的同名 `.txt`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessTexts {
    /// 标签：字段 `name`，它自己报的名字。
    pub message_open: String,
    /// 收尾。
    pub message_close: String,
}

impl HarnessTexts {
    /// 交给组装器的样子：标签读成模板，拿 `name` 试换一次。
    ///
    /// # Errors
    ///
    /// 标签的模板坏了，或者要了 `name` 以外的字段。
    pub(crate) fn rendered(&self) -> Result<Rendered, BuildError> {
        let bad = |error| BuildError::Texts {
            which: "harness message texts",
            error,
        };
        let open = Template::parse(&self.message_open).map_err(bad)?;
        open.render(&BTreeMap::from([("name", "")])).map_err(bad)?;
        Ok(Rendered {
            open,
            close: self.message_close.clone(),
        })
    }
}

#[cfg(test)]
mod tests;

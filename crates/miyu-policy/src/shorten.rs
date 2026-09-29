//! 截短重试装进快照的字和数（`docs/blueprint/compaction.md` 第三条第 10 条，施工 6-6 中）：截过的摘要请求前面补的那一条、
//! 摘要没看到的那一段的模板，和截几次、没说超了多少时截百分之几。以前造的快照里没有，读成没有：那些会话不截短，摘要请求
//! 超长照失败算。

use miyu_kernel::session::Shorten;
use miyu_kernel::template::Template;
use serde::{Deserialize, Serialize};

use crate::rebuild::template;
use crate::snapshot::BuildError;

/// 截短重试的字（`compaction/` 下）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortenTexts {
    /// 截过的摘要请求、留下的第一条是助手的，前面补的那一条 user（`truncated.txt`）。
    pub truncated: String,
    /// 摘要没看到的那一段（`notes-uncovered.txt`）：字段 `from`、`to`。
    pub notes_uncovered: String,
}

/// 截短重试的数（`compaction.md`「对外的样子」的策略数据）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShortenNumbers {
    /// 截着最多再试几次。
    pub tries: u32,
    /// 没说超了多少时，去掉剩下的组的百分之几。
    pub percent: u32,
}

/// 出厂的数（`09-压缩.md` Z7，照 Claude Code）：最多再试 3 次，没说超了多少的去掉 20%。
pub const SHORTEN: ShortenNumbers = ShortenNumbers {
    tries: 3,
    percent: 20,
};

impl ShortenNumbers {
    /// 交给内核的样子。
    pub(crate) fn kernel(self) -> Shorten {
        Shorten {
            tries: self.tries,
            percent: self.percent,
        }
    }
}

impl ShortenTexts {
    /// 摘要没看到的那一段的模板，试换过字段。
    ///
    /// # Errors
    ///
    /// 模板坏了。
    pub(crate) fn uncovered(&self) -> Result<Template, BuildError> {
        template(&self.notes_uncovered, &["from", "to"])
    }
}

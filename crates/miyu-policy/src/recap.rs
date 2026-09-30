//! 回顾装进快照（施工 3-8 四补，`docs/blueprint/kernel/request.md`「回顾的请求」）：`resources/core/recap/` 下的五份原文，和
//! 两个数。以前造的快照里没有，读成没有：那些会话不做回顾，要了是没有能回顾的。

use miyu_assemble::Recap;
use serde::{Deserialize, Serialize};

use crate::snapshot::Snapshot;

/// 回顾用的数（策略数据，照 codex 的 `RECAP_HISTORY_MAX_TURNS`、`RecapPrompt::MAX_ESTIMATED_TOKENS`）：造会话时冻结在快照里。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecapNumbers {
    /// 最多喂几轮她答过的。
    pub turns: u32,
    /// 整份（连指令）最多约多少 token，照本地估算的字节/4 折成字节。
    pub tokens: u64,
}

/// 出厂的回顾用的数：最多 8 轮，整份最多约 8192 个 token（2026-10-01 项目主人定，照 codex）。
pub const RECAP: RecapNumbers = RecapNumbers {
    turns: 8,
    tokens: 8_192,
};

/// 回顾的字（`recap/` 下，文件名是同名的 `.txt`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecapTexts {
    /// 指令，最后一行是 `Conversation:`。
    pub instruction: String,
    /// 人这边那一段的标签。
    pub user: String,
    /// 她的回答那一段的标签。
    pub assistant: String,
    /// 整轮去掉了最老的几轮时写在最前的那一行。
    pub omitted: String,
    /// 一段截了中间时夹在头尾之间的那一句。
    pub excerpted: String,
}

impl Snapshot {
    /// 交给组装器的回顾：字、数都有的才有，缺一样就不做回顾。
    pub(crate) fn recap(&self) -> Option<Recap> {
        let numbers = self.recap?;
        Some(self.core.recap.as_ref()?.rendered(numbers))
    }
}

impl RecapTexts {
    /// 交给组装器的样子：字照抄，数照快照里的。
    fn rendered(&self, numbers: RecapNumbers) -> Recap {
        Recap {
            instruction: self.instruction.clone(),
            user: self.user.clone(),
            assistant: self.assistant.clone(),
            omitted: self.omitted.clone(),
            excerpted: self.excerpted.clone(),
            turns: usize::try_from(numbers.turns).unwrap_or(usize::MAX),
            tokens: numbers.tokens,
        }
    }
}

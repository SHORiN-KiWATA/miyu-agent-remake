//! 起标题装进快照（施工 3-8 五补，`docs/blueprint/kernel/request.md`「起标题的请求」、`kernel/session.md`「起标题」）：
//! `resources/core/title/` 下的指令，和三个数。对话记录的标签、截断的记号借回顾的（`recap.rs`）。以前造的快照里没有，读成
//! 没有：那些会话不起标题。

use miyu_assemble::Title;
use miyu_kernel::session::Titles;
use serde::{Deserialize, Serialize};

use crate::snapshot::Snapshot;

/// 起标题用的数（策略数据）：造会话时冻结在快照里。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleNumbers {
    /// 整份请求（连指令）最多约多少 token，照本地估算的字节/4 折成字节。
    pub tokens: u64,
    /// 标题最多几个字（Unicode 字符），超了截掉。
    pub chars: u32,
    /// 一个会话最多试几次。
    pub tries: u32,
}

/// 出厂的起标题用的数：请求最多约 1024 个 token（照回顾的截法定的小上限，施工时定）；标题最多 50 个字（2026-10-01 项目主人
/// 定）；最多试两次（施工单）。
pub const TITLE: TitleNumbers = TitleNumbers {
    tokens: 1_024,
    chars: 50,
    tries: 2,
};

/// 起标题的字（`title/` 下，文件名是同名的 `.txt`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TitleTexts {
    /// 指令，最后一行是 `Conversation:`。
    pub instruction: String,
}

impl Snapshot {
    /// 交给组装器的起标题的字：字、数都有的才有。
    pub(crate) fn title(&self) -> Option<Title> {
        let numbers = self.title?;
        Some(Title {
            instruction: self.core.title.as_ref()?.instruction.clone(),
            tokens: numbers.tokens,
        })
    }

    /// 交给内核的起标题的两个数：字、数都有的才有，缺一样就不起标题。
    pub(crate) fn titles(&self) -> Option<Titles> {
        let numbers = self.title?;
        self.core.title.as_ref()?;
        Some(Titles {
            tries: numbers.tries,
            chars: usize::try_from(numbers.chars).unwrap_or(usize::MAX),
        })
    }
}

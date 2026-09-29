//! 熔断装进快照的数（`docs/blueprint/compaction.md` 第十条，施工 6-6 上）：连续失败几次、几个回合内又到线算快、连着快
//! 几次就暂停自动压缩。以前造的快照里没有，读成没有：那些会话不熔断。

use miyu_kernel::session::Pause;
use serde::{Deserialize, Serialize};

/// 熔断的数（`compaction.md`「对外的样子」的策略数据）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PauseNumbers {
    /// 自动压缩连续失败几次就暂停。
    pub failures: u32,
    /// 上一个检查点所在的那一轮算第 1 个回合，第几个以内又到线算快。
    pub turns: u32,
    /// 连着快几次就暂停。
    pub refills: u32,
}

/// 出厂的数（`09-压缩.md` Z7、Z9，照 Claude Code）：连续失败 3 次；压完 3 个回合内又到线，连着 3 次。
pub const PAUSE: PauseNumbers = PauseNumbers {
    failures: 3,
    turns: 3,
    refills: 3,
};

impl PauseNumbers {
    /// 交给内核的样子。
    pub(crate) fn kernel(self) -> Pause {
        Pause {
            failures: self.failures,
            turns: self.turns,
            refills: self.refills,
        }
    }
}

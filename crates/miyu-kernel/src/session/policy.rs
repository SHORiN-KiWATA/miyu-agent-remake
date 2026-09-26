//! 冻结在会话上的策略（`docs/designs/02-内核.md` K3，第六节「回合怎么开、请求怎么发」第 7 条）。

use std::fmt;

use crate::assemble::Assembler;
use crate::facts::FactTemplates;

/// 冻结在会话上的策略：造会话时由执行器照策略快照造好交进来（施工 3-6），会话里不再变。
pub struct Policy {
    /// 组装请求的做法，一个会话一种（`05-内核接口.md` 第五节「组装请求」）。会话是一个 actor，
    /// 会被挪到别的线程上跑（02 第七节），所以要 `Send`。
    pub assembler: Box<dyn Assembler + Send>,
    /// 两类事实的模板：环境和权限（`08-上下文投影.md` 第五节「环境和状态的事实怎么写」）。
    pub facts: FactTemplates,
}

/// 组装器是外面交进来的，不一定能打印，跳过它。
impl fmt::Debug for Policy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Policy")
            .field("facts", &self.facts)
            .finish_non_exhaustive()
    }
}

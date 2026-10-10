//! 分派（`docs/blueprint/chat.md` 第四条「怎么走」第 5、6 条，`docs/designs/18-通讯平台.md` 第八节，施工 O-9）：承诺要回的
//! 一条交给主线还是支线（[`dispatch`]）。一条线永远串行，要并行就分叉一条支线（18 第八节，Q24）。
//!
//! 纯逻辑：主线和支线各在回谁（[`Lines`]），由外面从场所会话的日志投影出来交进来；分叉以后支线怎么开、`reply-to` 怎么记，
//! 随桥和核心的那几步（「怎么走」第 7 条）。顶替在旁边的 `supersede`（施工时定的第 9 条，O-12 下）。

use miyu_kernel::id::ExternalId;

/// 一条线：主线或者一条支线。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// 闲着。
    Idle,
    /// 正在跑一轮，回的是这几个人（平台上的人的编号）。
    Busy {
        /// 这一轮要回的人。
        targets: Vec<ExternalId>,
    },
}

/// 这个场所的几条线（18 第八节）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lines {
    /// 主线：场所会话本身。
    pub main: Line,
    /// 正在跑的支线，照分叉的先后。
    pub lanes: Vec<Line>,
    /// 最多几条支线：场所规则的 `parallel`，`0` 就是串行。
    pub parallel: u8,
}

/// 分派的结论（「怎么走」第 5 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dispatch {
    /// 主线闲着：主线开一轮。
    StartMain,
    /// 并进主线这一轮，下一步听到。
    JoinMain,
    /// 并进第几条支线（[`Lines::lanes`] 里的位置，从 0 数）。
    JoinLane(usize),
    /// 分叉一条支线回他。
    Fork,
    /// 排到主线下一轮。
    Queue,
}

/// 承诺要回的一条分派到哪条线（「怎么走」第 5 条），终端管理员、白名单成员的 @ 直通的也一样。`sender` 是发的人。
///
/// 照这个先后：主线闲着开主线（不看支线，施工时定的第 3 条）；主线这一轮在回他就并进去（催一句「??」不该换来两条回复，
/// 旧版 09-26）；有支线在回他就并进第一条；支线比 `parallel` 少就分叉；都不行排到主线下一轮。
pub fn dispatch(sender: &ExternalId, lines: &Lines) -> Dispatch {
    if lines.main == Line::Idle {
        Dispatch::StartMain
    } else if lines.main.serves(sender) {
        Dispatch::JoinMain
    } else if let Some(index) = lines.lanes.iter().position(|lane| lane.serves(sender)) {
        Dispatch::JoinLane(index)
    } else if lines.lanes.len() < usize::from(lines.parallel) {
        Dispatch::Fork
    } else {
        Dispatch::Queue
    }
}

impl Line {
    /// 这条线这一轮在不在回这个人。
    fn serves(&self, sender: &ExternalId) -> bool {
        match self {
            Line::Idle => false,
            Line::Busy { targets } => targets.contains(sender),
        }
    }
}

#[cfg(test)]
mod tests;

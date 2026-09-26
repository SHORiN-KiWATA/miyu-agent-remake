//! 这一步的调用走到了哪（`docs/designs/02-内核.md` 第六节「工具怎么调、下一步怎么走」
//! 「确认怎么走」）：照调用的先后记着每个调用的状态，算出现在轮到的。
//!
//! 一个调用的一生：等着轮到它，过执行前的链，要问人的等人回答，人允许的等决定落了盘，派出去
//! 跑，有了结果。跑着的时候还可能问人一组题，等人回答，答完了等落了盘交给它（「提问怎么走」）。
//! 内核当场拦下的不在这里，它们已经有了结果。

use crate::event::{Question, Response};
use crate::id::{CallId, Seq};
use crate::tool::Access;

/// 这一步要跑的调用，照调用的先后。
#[derive(Debug)]
pub(super) struct Step {
    /// 回复的序号：它落了盘才派。
    pub(super) reply: Seq,
    /// 这一步的调用。
    pub(super) calls: Vec<Pending>,
}

/// 一个要跑的调用。
#[derive(Debug)]
pub(super) struct Pending {
    /// 调用编号。
    pub(super) id: CallId,
    /// 工具名。
    pub(super) name: String,
    /// 修正过的参数。
    pub(super) args: String,
    /// 工具的访问类别。
    pub(super) access: Access,
    /// 走到了哪。
    pub(super) state: State,
    /// 请人确认过的：请求里内核要看的两样。没请人确认过就没有。
    pub(super) asked: Option<Asked>,
    /// 在等人回答的那组题：查回答对不对得上。
    pub(super) questions: Vec<Question>,
    /// 人的回答：落了盘交给工具。
    pub(super) answers: Vec<Response>,
}

/// 请人确认时，请求里内核要看的两样。
#[derive(Debug)]
pub(super) struct Asked {
    /// 要的是哪一类访问：收紧成只读时，要写入的当场拦下。
    pub(super) access: Access,
    /// 提没提放行规则：没提的，只能选允许这一次或者拒绝。
    pub(super) rule: bool,
}

/// 一个调用走到了哪。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    /// 还没轮到它。
    Waiting,
    /// 交给了执行前的链，等结论。
    Guarding,
    /// 链说要问人，等人回答。
    Asking,
    /// 人允许了：等那条决定落了盘再派。
    Approved {
        /// 那条决定的序号。
        decided: Seq,
    },
    /// 派出去了，等结果。
    Running,
    /// 跑着的时候问了人一组题，等人回答。
    Questioning,
    /// 人答完了：等那条回答落了盘，交给工具。
    Answered {
        /// 那条回答的序号。
        answered: Seq,
    },
    /// 有了结果。
    Done,
}

impl Step {
    /// 现在轮到的，照调用的先后：连着的只读调用一起；不是只读的，等它前面的都有了结果，它没有
    /// 结果，后面的都等着。过链的、等人的，和在跑的一样占着位置。
    pub(super) fn ready(&self) -> Vec<usize> {
        let mut ready = Vec::new();
        let mut earlier_pending = false;
        let mut earlier_exclusive = false;
        for (k, call) in self.calls.iter().enumerate() {
            if call.state == State::Done {
                continue;
            }
            let exclusive = call.access != Access::Read;
            let free = if exclusive {
                !earlier_pending
            } else {
                !earlier_exclusive
            };
            if call.state == State::Waiting && free {
                ready.push(k);
            }
            earlier_pending = true;
            earlier_exclusive |= exclusive;
        }
        ready
    }

    /// 这一步的调用都有了结果。
    pub(super) fn finished(&self) -> bool {
        self.calls.iter().all(|call| call.state == State::Done)
    }

    /// 这一步里的 `call_id`，而且它正走到 `state`。
    pub(super) fn find(&mut self, call_id: CallId, state: State) -> Option<&mut Pending> {
        self.calls
            .iter_mut()
            .find(|call| call.id == call_id && call.state == state)
    }
}

impl Pending {
    /// 还没跑过：还没轮到、在过链、在等人、人允许了还没派。
    pub(super) fn not_run(&self) -> bool {
        matches!(
            self.state,
            State::Waiting | State::Guarding | State::Asking | State::Approved { .. }
        )
    }

    /// 在跑：派出去了，还没有结果。问着人、答完了等落盘的，也在跑。
    pub(super) fn executing(&self) -> bool {
        matches!(
            self.state,
            State::Running | State::Questioning | State::Answered { .. }
        )
    }

    /// 在等人回答：等确认的，和问着人的。这时来了一句话，就作废（「提问怎么走」第 5 条）。
    pub(super) fn awaiting_person(&self) -> bool {
        matches!(self.state, State::Asking | State::Questioning)
    }

    /// 要不要写入：工具是写文件的，或者请人确认的是写入。只读时拦下的就是这些。
    pub(super) fn writes(&self) -> bool {
        self.access.writes()
            || self
                .asked
                .as_ref()
                .is_some_and(|asked| asked.access.writes())
    }
}

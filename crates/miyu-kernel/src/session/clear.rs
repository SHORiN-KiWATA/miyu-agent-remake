//! 清空上下文（`docs/blueprint/compaction.md` 第十四条，施工 6-8 补）：`session.clear` 空闲时才收，单开一轮，压成一个空的
//! 检查点。
//!
//! 和手动压缩（`manual.rs`）一样单开一轮、`turn.started` 没有触发，只是不请求模型：开头、检查点、结束三条同一批追加。检查点
//! 替代到这一轮开头以前的最后一条，`summary` 是空的，没有代码写的几段、不重读文件；渲染时什么都不出（`kernel/request.md`
//! 「组装」第 2 条），下一轮开头照常注入环境、权限、会话编号几块事实（检查点后面没有，比不到）。
//!
//! 不数进熔断：没有摘要请求，不会失败。检查点换了，以前的失败、暂停都写在它前面，自动压缩跟着恢复（`breaker.rs`）。撤销
//! 那一轮，压缩跟着撤掉，上下文回到清空以前（第十一条，施工 6-9 的那一套）。

use super::action::{Action, Reason};
use super::turn::Stage;
use super::{Session, rejected};
use crate::event::{Body, CompactTrigger, ContextCompacted, EndReason, TurnStarted};
use crate::id::{CommandId, Seq};
use crate::origin::By;
use crate::time::Timestamp;

impl Session {
    /// 收下清空的命令：有回合在进行的拒绝，`turn_running`；上下文本来就是空的拒绝，`nothing_to_clear`。收下的，同一批追加
    /// `turn.started`（没有 `trigger`，`by` 是内核，`cause` 是这个命令，`cwd`、`dirs` 照会话现在的环境）、`context.compacted`
    /// （`trigger` 是 `clear`，替代到这一轮开头以前的最后一条，`summary` 是空的）、`turn.ended`（`completed`）。不追加事实、
    /// 不换实际生效的权限、不跑回合开始的挂接点；回合结束的挂接点照常跑。都落了盘才回应，附上 `turn.started` 的序号。
    pub(super) fn clear(&mut self, id: CommandId, at: Timestamp) -> Vec<Action> {
        if self.turn.is_some() {
            return vec![rejected(id, Reason::TurnRunning)];
        }
        // 替代到这一轮开头以前的最后一条：就是现在追加过的最后一条。
        let upto = Seq::new(self.ledger.next_seq().get() - 1);
        let Some(upto) = upto.filter(|_| !self.context_is_empty()) else {
            return vec![rejected(id, Reason::NothingToClear)];
        };
        let cause = Some(id.clone());
        let body = Body::TurnStarted(TurnStarted {
            trigger: None,
            triggers: Vec::new(),
            cwd: Some(self.environment.cwd.clone()),
            dirs: self.environment.dirs.clone(),
        });
        let started = self.record(at, By::Kernel, cause.clone(), body);
        // 这一轮当场就结束，只在这个函数里在进行。
        self.turn = Some(self.new_turn(started.seq, cause.clone(), Stage::Settling));
        // 记在一边的回报不再由它们另开一轮（施工 7-2）：随便哪一轮开了就清掉，它们也跟着清空了。
        self.deferred.clear();
        let body = Body::ContextCompacted(ContextCompacted {
            upto,
            summary: String::new(),
            trigger: Some(CompactTrigger::Clear),
            instructions: None,
            notes: String::new(),
            restored: Vec::new(),
            refills: None,
            prepared: false,
        });
        let compacted = self.record(at, By::Kernel, cause.clone(), body);
        let ended = self.end_turn(at, By::Kernel, cause, EndReason::Completed);
        self.accept(id, vec![started.seq]);
        vec![Action::Append(vec![started, compacted, ended])]
    }

    /// 上下文本来就是空的（`compaction.md` 第十四条第 2 条）：有效历史里没有检查点，或者最近的检查点是清空的，而且它后面
    /// 没有人的消息、回复、工具结果、回报、空了的通知（施工 C-6）。事实、回合的开头结尾不算：清了下一轮还是照样注入、照样有。
    fn context_is_empty(&self) -> bool {
        let summarized = self.history.checkpoint().is_some_and(|checkpoint| {
            !matches!(&checkpoint.body, Body::ContextCompacted(compacted)
                if compacted.trigger == Some(CompactTrigger::Clear))
        });
        !summarized
            && !self.history.events().iter().any(|event| {
                matches!(
                    event.body,
                    Body::MessageUser(_)
                        | Body::MessageAssistant(_)
                        | Body::ToolResult(_)
                        | Body::JobReported(_)
                        | Body::ChildReported(_)
                        | Body::PeerIdle(_)
                )
            })
    }
}

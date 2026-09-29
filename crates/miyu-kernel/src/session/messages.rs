//! 父子之间的留言，内核这一头（施工 7-7，`docs/blueprint/agents.md` 第六条，`docs/blueprint/kernel/session.md`「子代理的
//! 留言」）。
//!
//! 父会话发给子代理的留言就是父会话发来的 `message.user`，和交代一样走「发一条消息」：内核不另认。子代理发给父会话的留言，
//! 发命令的是这个会话派的子代理（`by` 是它的子会话），照回报的规矩到（`jobs.rs` 的 [`Session::land`]）：不带回合编号，
//! 闲着、这时开得了由它开一轮，正忙排进这一轮的回报队、下一步看到，没人看着的一次性会话只记下；派它的那一轮撤掉了的不叫醒。
//! 它不是人说的话：打断时不撤回，不作废在等人的调用，撤销时留着。
//!
//! 执行器派 `message_agent` 的调用之前，照 [`Session::subagents`] 抄一份这个会话派出去的子代理，工具照它认 `to`。

use std::collections::BTreeMap;

use super::Session;
use super::action::Action;
use crate::block::Block;
use crate::event::{Body, MessageUser};
use crate::id::{CommandId, JobId, SessionId};
use crate::ledger::Ledger;
use crate::origin::By;
use crate::time::Timestamp;

/// 这个会话派出去的一个子代理（施工 7-7）：`message_agent` 发给它之前照它认。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subagent {
    /// 它的子会话：留言发到这里。
    pub session: SessionId,
    /// 被停掉了（以 `stopped`、`undone` 报过）：不再收留言。
    pub stopped: bool,
}

impl Session {
    /// 这个会话派出去的子代理，照编号（施工 7-7）：派它的那一轮撤掉了的不在（她看不到派它的那次调用，也就不是她的）。
    /// 做完了、崩了报过的照样在：它的会话还在，留言开它的下一轮。纯查询：执行器派每一次调用之前抄一份交给 `message_agent`。
    pub fn subagents(&self) -> BTreeMap<JobId, Subagent> {
        self.ledger
            .subagents()
            .filter(|(job, _, _)| {
                self.history
                    .dispatched(job)
                    .is_some_and(|dispatched| !dispatched.undone)
            })
            .map(|(job, session, stopped)| {
                let session = session.clone();
                (job, Subagent { session, stopped })
            })
            .collect()
    }

    /// 发消息的是这个会话派的子代理：交回它的任务编号。
    pub(super) fn subagent_sending(&self, by: &By) -> Option<JobId> {
        sent_by(&self.ledger, by)
    }

    /// 子代理 `job` 发来的留言：记一条 `message.user`，`by` 是它的子会话，`cause` 是这个命令，不带回合编号；照回报的规矩
    /// 叫不叫醒她。落了盘回应，附上这一条的序号。
    ///
    /// # Panics
    ///
    /// 过不了账本：不带回合编号的 `message.user` 哪条规矩都不犯，过不了只会是内核自己的 bug。
    pub(super) fn subagent_says(
        &mut self,
        id: CommandId,
        by: By,
        at: Timestamp,
        blocks: Vec<Block>,
        job: JobId,
    ) -> Vec<Action> {
        let body = Body::MessageUser(MessageUser { blocks });
        let events = match self.land(at, by, Some(id.clone()), body, Some(job)) {
            Ok(events) => events,
            Err(error) => panic!("a subagent's message failed the ledger, a kernel bug: {error}"),
        };
        self.accept(id, vec![events[0].seq]);
        vec![Action::Append(events)]
    }
}

/// `by` 是这个会话派的子代理（它的子会话）：交回任务编号。被停掉的、撤掉的回合里派的也认；别的没有。
pub(super) fn sent_by(ledger: &Ledger, by: &By) -> Option<JobId> {
    match by {
        By::Session(session) => ledger.subagent_in(&session.id),
        _ => None,
    }
}

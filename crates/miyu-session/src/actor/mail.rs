//! 人的那条收件箱里的一封怎么办（`docs/blueprint/session/actor.md` 第 3 条第 4 点）：命令照 actor 的时钟记下到的时刻送进
//! 内核，订阅当场办；拿着订阅的头从没有到有、从有到没有，交内核 `Watched`（施工 7-9）。施工 7-9 从 `actor.rs` 挪出来。

use tokio::sync::oneshot;

use miyu_kernel::session::{Input, Reason, Received};

use super::{Actor, answer};
use crate::handle::{Halt, Message};

/// 收件箱里的一封怎么办。
pub(super) enum Mail {
    /// 送进内核。
    Input(Input),
    /// 当场办完了。
    Done,
    /// 有计划地停下，停好了回这一头。
    Stop(oneshot::Sender<()>),
    /// 停掉派出去的任务（施工 7-4）：要等杀掉、存好，在 `halt.rs` 里办。
    Halt(Halt),
    /// 删会话之前停下（施工 3-8 三补，`stop.rs`）。
    Delete(bool, oneshot::Sender<Result<(), Reason>>),
}

impl Actor {
    /// 收件箱里的一封：命令照 actor 的时钟记下到的时刻，订阅当场办，看着的头有没有变了送进内核（施工 7-9）。
    pub(super) fn mail(&mut self, message: Message) -> Mail {
        match message {
            Message::Command {
                id,
                by,
                command,
                reply,
            } => {
                self.wait_for(id.clone(), reply);
                let at = self.clock.now();
                Mail::Input(Input::Command(Received {
                    id,
                    by,
                    at,
                    command,
                }))
            }
            Message::Subscribe(reply) => {
                answer(reply, self.pushes.subscribe());
                self.watch(true)
            }
            Message::Unsubscribed => self.watch(false),
            Message::Stop(reply) => Mail::Stop(reply),
            Message::Halt(halt) => Mail::Halt(halt),
            Message::Delete { force, reply } => Mail::Delete(force, reply),
            Message::Environment(environment) => {
                self.tools.locate(environment.offset);
                Mail::Input(Input::Environment(environment))
            }
        }
    }

    /// 多了（`more`）或者少了一个拿着订阅的头（施工 7-9）：从没有到有、从有到没有，交内核 `Watched`；别的当场办完。
    fn watch(&mut self, more: bool) -> Mail {
        let before = self.watchers;
        self.watchers = match more {
            true => before + 1,
            false => before.saturating_sub(1),
        };
        match (before, self.watchers) {
            (0, 1) => Mail::Input(Input::Watched { watched: true }),
            (1, 0) => Mail::Input(Input::Watched { watched: false }),
            _ => Mail::Done,
        }
    }
}

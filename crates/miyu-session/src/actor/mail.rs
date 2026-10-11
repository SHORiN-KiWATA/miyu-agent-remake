//! 人的那条收件箱里的一封怎么办（`docs/blueprint/session/actor.md` 第 3 条第 4 点）：命令照 actor 的时钟记下到的时刻送进
//! 内核，订阅当场办（连同补发补到哪一条，施工 3-8 六补）；拿着订阅的头从没有到有、从有到没有，交内核 `Watched`
//! （施工 7-9），和 `Handle` 共用的旗一起写（施工 C-5）。施工 7-9 从 `actor.rs` 挪出来。
//! 头读后台命令的输出（施工 7-4 补）不进内核：另起一个任务读，当场办完。别的会话等这个会话空下来（施工 C-6）也不进内核：
//! 记进名单，当场办完。

use std::sync::atomic::Ordering;
use std::time::Instant;

use tokio::sync::oneshot;

use miyu_kernel::id::Seq;
use miyu_kernel::session::{Input, Reason, Received};

use super::{Actor, answer};
use crate::handle::{Halt, Message, Taken};

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
    /// 闲够了，答了会话表，退出（施工 V-2 再补，`life.rs`）。
    Retired,
}

impl Actor {
    /// 收件箱里的一封：命令照 actor 的时钟记下到的时刻，订阅当场办，看着的头有没有变了送进内核（施工 7-9）。
    pub(super) fn mail(&mut self, message: Message) -> Mail {
        if !matches!(message, Message::Retire { .. }) {
            self.quiet = Instant::now();
        }
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
            // 补发补到哪一条和订阅在这同一步里拿（施工 3-8 六补）：这一步之前落了盘的都推过了，之后的都还没推。
            Message::Subscribe(reply) => {
                let taken = Taken {
                    pushes: self.pushes.subscribe(),
                    upto: self.session.landed().map_or(0, Seq::get),
                    log: self.tools.log(),
                    current: self.current(),
                };
                answer(reply, taken);
                self.watch(true)
            }
            Message::Current(reply) => {
                answer(reply, self.current());
                Mail::Done
            }
            Message::Unsubscribed => self.watch(false),
            Message::Stop(reply) => Mail::Stop(reply),
            Message::Halt(halt) => Mail::Halt(halt),
            Message::Delete { force, reply } => Mail::Delete(force, reply),
            Message::Output { job, reply } => {
                // 是什么当场照名册看；开文件另起一个任务，不在收件箱里等（施工 7-4 补）。
                let reading = self.jobs.command_output(job);
                tokio::spawn(async move { answer(reply, reading.await) });
                Mail::Done
            }
            // 有会话在等这个会话空下来（施工 C-6，`watchers.rs`）：不进内核，记进名单，上不上膛照 `watchers.rs` 判
            // （2026-10-01 改：空着的先不发，等下一次忙完）。
            Message::Watch { watcher, since } => {
                self.add_waiter(watcher, since);
                Mail::Done
            }
            // 人叫她现在就整理记忆（施工 R-7 补，`extract.rs`）：先抽再合，在后台，合完交回去。
            Message::Dream(reply) => {
                self.dream(reply);
                Mail::Done
            }
            Message::Retire { idle, reply } => self.retire(idle, reply),
        }
    }

    /// 多了（`more`）或者少了一个拿着订阅的头（施工 7-9）：从没有到有、从有到没有，交内核 `Watched`，和 `Handle` 共用的
    /// 那面旗一起写（施工 C-5，照 `busy` 的做法）；别的当场办完。
    fn watch(&mut self, more: bool) -> Mail {
        let before = self.watchers;
        self.watchers = match more {
            true => before + 1,
            false => before.saturating_sub(1),
        };
        self.watched.store(self.watchers > 0, Ordering::Release);
        match (before, self.watchers) {
            (0, 1) => Mail::Input(Input::Watched { watched: true }),
            (1, 0) => Mail::Input(Input::Watched { watched: false }),
            _ => Mail::Done,
        }
    }
}

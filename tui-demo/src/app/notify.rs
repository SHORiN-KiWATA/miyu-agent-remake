//! 界面这一边什么时候通知（蓝图 `tui.md`「系统通知」第 1、6 条）：一轮结束弹「回答好了」「出错了」，抽屉打开弹
//! 「在等你确认」「在等你回答」；在 herdr 里报在做、在等、空闲。弹不弹、响不响由 `notify::plan` 定。

use super::App;
use crate::core::{EndReason, Push, Update};
use crate::notify::{Event, State};

impl App {
    /// 核心的一条消息过一遍通知，在正文收它之前：没有在跑的一轮的结束不弹（核心重启以后补推的），手动压缩、清空那一轮
    /// 不弹。
    pub(super) fn notify_core(&mut self, update: &Update) {
        let seen = self.transcript.running.is_some();
        let manual = self.transcript.manual_turn();
        match update {
            Update::Push(Push::TurnStarted(..)) => self.notifier.state(State::Working),
            Update::Push(Push::TurnEnded(reason)) => {
                if seen && !manual {
                    match reason {
                        EndReason::Completed => self.notifier.tell(Event::Replied),
                        EndReason::Error => self.notifier.tell(Event::Failed),
                        EndReason::Interrupted | EndReason::Other(_) => {}
                    }
                }
                // 一轮结束：还有开着的抽屉的在等，不然空闲。
                let state = if self.drawers.open() {
                    State::Blocked
                } else {
                    State::Idle
                };
                self.notifier.state(state);
            }
            Update::Disconnected => self.notifier.state(State::Idle),
            _ => {}
        }
    }

    /// 抽屉开了一个（新开的、了结一个轮到下一个的）：弹「在等你确认」「在等你回答」，报在等。
    pub(super) fn notify_drawer(&mut self) {
        let Some(drawer) = self.drawers.current.as_ref() else {
            return;
        };
        let event = if drawer.is_question() {
            Event::Asking
        } else {
            Event::Approving
        };
        self.notifier.tell(event);
        self.notifier.state(State::Blocked);
    }

    /// 没有在等你的了：一轮还在跑的在做，不然空闲；还有开着的抽屉的在等。
    pub(super) fn settle_state(&mut self) {
        let state = if self.drawers.open() {
            State::Blocked
        } else if self.transcript.running.is_some() {
            State::Working
        } else {
            State::Idle
        };
        self.notifier.state(state);
    }
}

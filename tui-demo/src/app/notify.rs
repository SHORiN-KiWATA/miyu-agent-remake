//! 界面这一边什么时候通知（蓝图 `tui.md`「系统通知」第 1、6 条）：一轮结束弹「回答好了」「出错了」，抽屉打开弹
//! 「在等你确认」「在等你回答」；在 herdr 里报在做、在等、空闲。弹不弹、响不响由 `notify::plan` 定。

use super::App;
use crate::core::{EndReason, Push, Update};
use crate::notify::{Event, State};

impl App {
    /// 核心的一条消息过一遍通知，在正文收它之前：没有在跑的一轮的结束不弹（核心重启以后补推的），手动压缩、清空那一轮
    /// 不弹。
    pub(super) fn notify_core(&mut self, update: &Update) {
        // 补发来的是以前的事：不弹、不响、不报；补完了照这时在不在跑报一次 herdr（「会话列表」第 6 条）。
        if self.transcript.replaying() {
            if let Update::Push(Push::Clock(None)) = update {
                let running = self.transcript.running.is_some();
                self.notifier
                    .state(herdr_state(self.drawers.open(), running));
            }
            return;
        }
        let seen = self.transcript.running.is_some();
        let manual = self.transcript.manual_turn();
        match update {
            // 一轮开始：有抽屉开着的照旧在等（别处来的话可以在抽屉开着时开一轮）。
            Update::Push(Push::TurnStarted(..)) => {
                self.notifier.state(herdr_state(self.drawers.open(), true));
            }
            Update::Push(Push::TurnEnded(reason)) => {
                if seen && !manual {
                    match reason {
                        EndReason::Completed => self.notifier.tell(Event::Replied),
                        EndReason::Error => self.notifier.tell(Event::Failed),
                        EndReason::Interrupted | EndReason::Other(_) => {}
                    }
                }
                // 一轮结束：还有开着的抽屉的在等，不然空闲。
                self.notifier.state(herdr_state(self.drawers.open(), false));
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
        let state = herdr_state(self.drawers.open(), self.transcript.running.is_some());
        self.notifier.state(state);
    }
}

/// 在 herdr 里报哪种（蓝图「系统通知」第 6 条）：有抽屉开着就是在等，在等你比在做要紧；不然一轮在跑是在做，都没有是空闲。
fn herdr_state(drawer_open: bool, running: bool) -> State {
    if drawer_open {
        State::Blocked
    } else if running {
        State::Working
    } else {
        State::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::herdr_state;
    use crate::notify::State;

    #[test]
    fn an_open_drawer_outranks_a_running_turn() {
        // 2026-10-01 查出来：别处来的话在抽屉开着时开了一轮，原来报成在做，一轮结束才改回在等。
        assert_eq!(herdr_state(true, true), State::Blocked);
        assert_eq!(herdr_state(true, false), State::Blocked);
        assert_eq!(herdr_state(false, true), State::Working);
        assert_eq!(herdr_state(false, false), State::Idle);
    }
}

//! 收尾（施工 3-9 下，施工 7-9 从 `follow.rs` 挪出来）：一轮结束时收好这一轮的屏幕；都结束了印用量（跟过的每一轮加起来）、
//! 有几步因为要确认没做的那一句、说为什么结束的那一句（照最后一轮），交回退出码。等子代理的时候不等了（Ctrl+C、`--timeout`）
//! 也在这里收尾。

use serde_json::json;

use super::{Follow, Format, Screen, Step, steps};
use crate::ask::exit;
use crate::shown::{GRAY, Line, RESET, say, write};

/// 为什么不等了（施工 7-9）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Leaving {
    /// 等子代理的时候按了 Ctrl+C。
    Pressed,
    /// 到了 `--timeout`。
    TimedOut,
}

impl Follow<'_> {
    /// 一轮结束了，原因是 `reason`：收好这一轮的屏幕，记下它的回答和用量。等子代理的、这一轮不是打断结束的，还有没报的，
    /// 或者回报要叫醒她开下一轮的，接着等（施工 7-9）；别的收尾。
    pub(super) fn ended(&mut self, reason: &str, screen: &mut Screen<'_>) -> Step {
        self.close_turn(screen);
        self.turns
            .push(json!({"text": self.answer, "usage": self.usage.json()}));
        self.turn = None;
        self.reason = reason.to_string();
        self.agents.turn_ended(reason);
        // 接上的那一轮没听到这一句（施工 7-10，`joining.rs`）：同一批接着开的下一轮才是，不收尾，也不算第一轮结束了。
        if self.joining.missed() {
            return Step::Going;
        }
        self.ended_first = true;
        if self.waits && reason != "interrupted" && !self.agents.settled() {
            self.wait_line(screen);
            return Step::Going;
        }
        Step::Done(self.conclude(screen))
    }

    /// 收好这一轮的屏幕：只想了、没回答的（例如被打断了），思考那一行收个尾；回答末尾补上换行；最后是一块、一段思考的，
    /// 后面欠着空行。
    pub(super) fn close_turn(&mut self, screen: &mut Screen<'_>) {
        if self.plan.format != Format::Text {
            return;
        }
        self.part(screen, "");
        self.close_answer(screen);
        self.thought();
    }

    /// Ctrl+C、`--timeout` 打不打断她那一轮：本人说的打断；别的 harness 说的（`--from`，施工 7-10，2026-09-30 主会话定）不
    /// 打断，只是不等了：那一轮是她的，会话是人的。
    pub(crate) fn interrupts(&self) -> bool {
        self.plan.from.is_none()
    }

    /// 都结束了：印用量，有几步因为要确认没做的说一句，说为什么结束，交回退出码。照最后一轮怎么结束的算：照常结束、又有
    /// 几步没做的是 4；被打断、出错、没有模型的照旧，它们比 4 要紧。
    pub(super) fn conclude(&mut self, screen: &mut Screen<'_>) -> u8 {
        let language = self.plan.language;
        let (code, note) = match (self.reason.as_str(), &self.failure) {
            ("completed", _) if self.unattended > 0 => (exit::UNATTENDED, None),
            ("completed", _) => (exit::OK, None),
            ("interrupted", _) => (exit::INTERRUPTED, Some(language.interrupted())),
            ("error", Some(failure)) if failure.class == "auth" && !failure.sent => {
                (exit::NO_MODEL, Some(language.no_model()))
            }
            ("error", Some(failure)) => (
                exit::ERROR,
                Some(language.failed(&failure.class, &failure.message)),
            ),
            ("error", None) => (exit::ERROR, Some(language.failed("other", ""))),
            (other, _) => (exit::ERROR, Some(language.unfinished(other))),
        };
        self.close(note.is_some(), screen);
        if let Some(note) = note {
            say(screen.err, &note);
        }
        code
    }

    /// 不等了（施工 7-9）：等子代理的时候按了 Ctrl+C，或者到了 `--timeout`（有回合在进行的，头已经叫它打断了）。写了
    /// `--from` 的，她那一轮还在进行时按 Ctrl+C 也是不等了，两样都不打断她（施工 7-10，[`Follow::interrupts`]）。收好屏幕，
    /// 照常印用量、最后那一句，再说一句为什么不等了；交回 3。
    pub(crate) fn leave(&mut self, why: Leaving, screen: &mut Screen<'_>) -> u8 {
        let language = self.plan.language;
        let running = !self.interrupts() && !self.waiting();
        let line = match (why, running) {
            (Leaving::Pressed, true) => language.left_running(false),
            (Leaving::TimedOut, true) => language.left_running(true),
            (Leaving::Pressed, false) => language.stopped_waiting(),
            (Leaving::TimedOut, false) => language.timed_out(self.agents.owed() > 0),
        };
        self.unwait(screen);
        self.close_turn(screen);
        self.close(true, screen);
        match self.plan.format {
            Format::Text => write(screen.err, &Line::gray(line).paint(screen.gray)),
            Format::Json => say(screen.err, &line),
        }
        exit::INTERRUPTED
    }

    /// 印跟过的每一轮加起来的用量、有几步因为要确认没做的那一句；给脚本的印一行 JSON，照先后列出结束了的每一轮。`more` 是
    /// 后面还有一句要印：一块、一段思考后面欠着的空行先写上。
    fn close(&mut self, more: bool, screen: &mut Screen<'_>) {
        match self.plan.format {
            Format::Text => {
                if self.total.seen || self.unattended > 0 || more {
                    self.settle(screen);
                }
                if self.total.seen {
                    let line = self.plan.language.usage(&self.total);
                    match screen.gray {
                        true => write(screen.err, &format!("{GRAY}{line}{RESET}\n")),
                        false => write(screen.err, &format!("{line}\n")),
                    }
                }
                if self.unattended > 0 {
                    let line = steps::unattended_line(self.plan, self.unattended);
                    write(screen.err, &line.paint(screen.gray));
                }
            }
            Format::Json => {
                let mut result = json!({"session": self.session, "turns": self.turns});
                if let Some(failure) = &self.failure {
                    result["error"] = json!({"class": failure.class, "message": failure.message});
                }
                write(screen.out, &format!("{result}\n"));
            }
        }
    }
}

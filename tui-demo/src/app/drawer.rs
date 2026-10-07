//! 确认和提问的抽屉接到程序上（蓝图 `tui.md`「确认和提问的抽屉」）：假事件打开、按键先归它、两下 `Esc` 取消、
//! 了结以后正文里留下结果。

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::App;
use crate::commands::Run;
use crate::core::Command;
use crate::drawer::{Drawer, Edit, Mark, Outcome, Report, Step};
use crate::transcript::{JobMark, Kind};

impl App {
    /// `/demo-ask`、`/demo-approve`：照 `fake.json` 轮着出一个（第 1 条）。
    pub(super) fn demo_drawer(&mut self, run: Run) {
        let fake = &self.config.fake;
        let drawer = match run {
            Run::DemoAsk if !fake.asks.is_empty() => {
                let f = &fake.asks[self.demo_drawers.0 % fake.asks.len()];
                self.demo_drawers.0 += 1;
                Drawer::question(f.who.clone(), f.body.clone())
            }
            Run::DemoApprove if !fake.approvals.is_empty() => {
                let f = &fake.approvals[self.demo_drawers.1 % fake.approvals.len()];
                self.demo_drawers.1 += 1;
                Drawer::approval(f.who.clone(), f.body.clone())
            }
            _ => return,
        };
        let mut drawer = drawer;
        drawer.demo = true;
        let was_open = self.drawers.open();
        self.drawers.push(drawer);
        // 新开的抽屉：弹「在等你」（「系统通知」第 1 条）；前一个还没了结的排着，轮到它时再弹。
        if !was_open {
            self.notify_drawer();
        }
    }

    /// 抽屉开着时按键先归它，带 `Ctrl` 的照旧归外面（复制、退出），编辑时的 `Ctrl+J` 换行除外（第 4 条）。
    /// 拿了交回 `true`。
    pub(super) fn drawer_key(&mut self, key: KeyEvent) -> bool {
        if !self.drawers.open() {
            return false;
        }
        let newline = self.drawers.editing() && key.code == KeyCode::Char('j');
        if key.modifiers.contains(KeyModifiers::CONTROL) && !newline {
            return false;
        }
        let window = self.esc_window();
        let step = self
            .drawers
            .current
            .as_mut()
            .map_or(Step::Stay, |d| match d.key(key) {
                Step::Escape => d.escape(Instant::now(), window),
                step => step,
            });
        self.drawer_step(step);
        true
    }

    /// 抽屉开着时粘贴：正在写的（「其他」、补充、理由）接进去；没在编辑不收。
    pub(super) fn drawer_paste(&mut self, text: &str) {
        let Some(d) = self.drawers.current.as_mut() else {
            return;
        };
        let tab = d.tab;
        match d.editing {
            Some(Edit::Notes) => d.notes[tab].push_str(text),
            Some(Edit::Other | Edit::Reason) => d.typed[tab].push_str(text),
            None => {}
        }
    }

    /// 点中抽屉的第几项。
    pub(super) fn drawer_click(&mut self, index: usize) {
        let step = self
            .drawers
            .current
            .as_mut()
            .map_or(Step::Stay, |d| d.click(index));
        self.drawer_step(step);
    }

    /// 按过一下 `Esc` 的提示什么时候到点消失（主循环照它醒来重画）。
    pub(super) fn drawer_deadline(&self) -> Option<Instant> {
        let at = self.drawers.current.as_ref()?.armed?;
        Some(at + self.esc_window())
    }

    /// 到点了：按过一下 `Esc` 过了时限，提示换回按键提示。
    pub(super) fn drawer_tick(&mut self) {
        let window = self.esc_window();
        if let Some(d) = self.drawers.current.as_mut()
            && !d.armed(Instant::now(), window)
        {
            d.armed = None;
        }
    }

    /// 两下 `Esc` 之间的时限：和别处的两下 `Esc` 一样。
    fn esc_window(&self) -> Duration {
        Duration::from_millis(self.config.layout.esc_window_ms)
    }

    /// 按了一下以后：了结的留下结果。
    fn drawer_step(&mut self, step: Step) {
        if let Step::Done(outcome) = step {
            self.settle_drawer(&outcome);
        }
    }

    /// 了结：交了的经 `session.answer` 交给问的那个会话，正文的结果等核心推来回答再写（`asking.rs`）；演示的、取消的
    /// 当场写：提问答了是引用块，不允许、取消是一行，允许了什么都不留（第 6 条）；取消的在回答时顺带打断这一轮（第 5 条）。
    fn settle_drawer(&mut self, outcome: &Outcome) {
        let Some(drawer) = self.drawers.finish() else {
            return;
        };
        let answered = match outcome {
            Outcome::Answered(a) => Some(serde_json::json!({"answers": a.answers})),
            Outcome::Decided(d) => {
                Some(serde_json::json!({"decision": d.decision, "reason": d.reason}))
            }
            Outcome::Cancelled => None,
        };
        match answered.filter(|_| !drawer.demo) {
            Some(mut body) => {
                if body["reason"].is_null()
                    && let Some(fields) = body.as_object_mut()
                {
                    fields.remove("reason");
                }
                // 交给问的那个会话：`/new`、切会话以后还开着的抽屉也不会交错地方。
                let owner = Some(drawer.owner.clone()).filter(|o| !o.is_empty());
                self.core.send(Command::Answer {
                    session: owner.or_else(|| drawer.session.clone()),
                    call: drawer.call_id.clone(),
                    body,
                });
            }
            None => self.write_report(&drawer, outcome),
        }
        if *outcome == Outcome::Cancelled && self.transcript.running.is_some() {
            self.core.send(Command::Interrupt { send: true });
        }
        // 轮到下一个抽屉的弹「在等你」，都了结了回到在做或空闲（「系统通知」第 1、6 条）。
        if self.drawers.open() {
            self.notify_drawer();
        } else {
            self.settle_state();
        }
    }
}

impl App {
    /// 正文末尾写一问的结果（第 6 条）。
    pub(super) fn write_report(&mut self, drawer: &crate::drawer::Drawer, outcome: &Outcome) {
        match drawer.report(outcome, &self.config.text.drawer) {
            Report::Block(lines) => self.transcript.note(Kind::Answered, lines.join("\n")),
            Report::Line(mark, text) => self.transcript.job(job_mark(mark), text, String::new()),
            Report::Nothing => {}
        }
    }
}

/// 了结那一行的记号：不允许是红 `✗`，取消和后台命令停了一样是暗 `●`（第 6 条）。
fn job_mark(mark: Mark) -> JobMark {
    match mark {
        Mark::Bad => JobMark::Failed,
        Mark::Void => JobMark::Stopped,
    }
}

#[cfg(test)]
mod tests {
    use super::job_mark;
    use crate::drawer::Mark;
    use crate::transcript::JobMark;

    #[test]
    fn a_cancelled_drawer_leaves_a_dim_dot_like_a_stopped_job() {
        // 2026-10-01 项目主人：「已取消」加点。
        assert_eq!(job_mark(Mark::Void), JobMark::Stopped);
        assert_eq!(job_mark(Mark::Bad), JobMark::Failed);
    }
}

//! 等子代理（施工 7-9，`docs/blueprint/cli/ask.md`「等子代理」，样子 2026-09-30 项目主人定）：第一轮结束了还有子代理没报的，
//! 接着跟被回报叫醒的几轮，都了结了才收尾。
//!
//! - 等的时候印一行灰字 `· 等 <N> 个子代理回报…（按 Ctrl+C 不等了）`：标准错误是终端的原地刷新，数目变了重画，别的东西来了
//!   先擦掉；不是终端的只印一次。
//! - 这次派出去的子代理报回来了，印一行灰字 `· <编号>「<标题>」报回来了`：回报和它叫醒的那一轮同一批到，这一行就在那一轮
//!   前面；一轮里到的，印在那一步前后。

use serde_json::{Value, json};

use super::{Follow, Format, REDRAW, Screen, Step, steps};
use crate::shown::{Line, write};

/// 等的那一行（施工 7-9）。
#[derive(Debug, Default)]
pub(super) struct Waiting {
    /// 终端里画着、还没擦掉：画之前屏幕上最后一行是不是空行、有没有还没和回答隔开的旁白，擦掉时照回去。
    drawn: Option<(bool, bool)>,
    /// 不是终端的：印过一次了。
    said: bool,
}

impl Follow<'_> {
    /// 在等子代理：第一轮结束了，这时没有跟着的回合在进行。按 Ctrl+C 是不等了，不是打断。
    pub(crate) fn waiting(&self) -> bool {
        self.waits && self.ended_first && self.turn.is_none()
    }

    /// 一轮开了头（`turn.started` 整条事件）：第一轮是自己发的那条命令开的（她正忙时由别人开的、会听到这一句的那一轮，
    /// 在它后面的推送里接上，施工 7-10，`joining.rs`）；第一轮结束以后，等子代理的时候开的每一轮都跟（被回报叫醒的，施工
    /// 7-9）。有跟着的回合在进行的不理。
    pub(super) fn started(&mut self, event: &Value, screen: &mut Screen<'_>) {
        let ours = match self.ended_first {
            false => event["cause"] == json!(self.sent),
            true => self.waits,
        };
        if self.turn.is_some() || !ours {
            return;
        }
        self.begin(event["turn"].as_u64(), false, screen);
    }

    /// 跟第 `turn` 轮：这一轮的回答、用量、出错从头记。`joined` 是它不是这一句开的，是接上的（施工 7-10）。
    pub(super) fn begin(&mut self, turn: Option<u64>, joined: bool, screen: &mut Screen<'_>) {
        self.unwait(screen);
        self.agents.turn_started();
        self.kinds.clear();
        self.answer.clear();
        self.stepped = false;
        self.usage = super::Sum::default();
        self.failure = None;
        self.compacting = super::compacting::Compacting::default();
        self.turn = turn;
        self.joining.begin(joined);
    }

    /// 子代理的回报（`child.reported` 整条事件）：这次派出去的，印一行报回来了。闲着时到的，都了结了就收尾；回报叫醒她的，
    /// 等那一轮；还有没报的，刷新等的那一行。不等子代理的不理。
    pub(super) fn child_reported(&mut self, event: &Value, screen: &mut Screen<'_>) -> Step {
        if !self.waits {
            return Step::Going;
        }
        let running = self.turn.is_some();
        let Some((job, title)) = self.agents.reported(event, running) else {
            return Step::Going;
        };
        if self.plan.format == Format::Text {
            self.unwait(screen);
            let (job, title) = (steps::one_line(&job), steps::one_line(&title));
            let line = Line::gray(self.plan.language.reported(&job, &title));
            self.aside(&line, screen);
        }
        if running || !self.ended_first {
            return Step::Going;
        }
        if self.agents.settled() {
            return Step::Done(self.conclude(screen));
        }
        self.wait_line(screen);
        Step::Going
    }

    /// 还有子代理没报、也没有回报叫醒的一轮要来：印等的那一行。终端里原地刷新，不换行，数目变了重画；不是终端的只印一次。
    /// `--format json` 不印。
    pub(super) fn wait_line(&mut self, screen: &mut Screen<'_>) {
        let Some(count) = self.agents.shown() else {
            return;
        };
        if self.plan.format != Format::Text {
            return;
        }
        let line = Line::gray(self.plan.language.waiting(count));
        if !screen.live {
            if !self.waiting.said {
                self.waiting.said = true;
                self.aside(&line, screen);
            }
            return;
        }
        if self.waiting.drawn.is_none() {
            self.close_answer(screen);
            self.thought();
            self.settle(screen);
            if !self.err_ends_line {
                write(screen.err, "\n");
                self.err_ends_line = true;
            }
            self.waiting.drawn = Some((self.blank, self.aside));
        }
        let painted = line.paint(screen.gray);
        write(
            screen.err,
            &format!("{REDRAW}{}", painted.trim_end_matches('\n')),
        );
        self.err_ends_line = false;
        self.aside = true;
        self.blank = false;
    }

    /// 终端里画着等的那一行的，擦掉：光标回到那一行的行首，屏幕照画它之前的样子接着印。
    pub(super) fn unwait(&mut self, screen: &mut Screen<'_>) {
        if let Some((blank, aside)) = self.waiting.drawn.take() {
            write(screen.err, REDRAW);
            self.err_ends_line = true;
            self.blank = blank;
            self.aside = aside;
        }
    }
}

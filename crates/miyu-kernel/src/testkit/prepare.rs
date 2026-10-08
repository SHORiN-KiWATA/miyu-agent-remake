//! 替身的提前压好（施工 6-11 上，`docs/blueprint/compaction.md` 第十五条）：回合开始交给内核的开关，提前压的请求怎么回、
//! 停住的放行。和起标题一样是内核自己要的旁路请求：没排剧本的只记下、不回（一直在路上），不管它的测试不用替它排。

use std::collections::VecDeque;

use super::Stage;
use super::aside::partway;
use super::script::Line;
use crate::event::Purpose;
use crate::id::Seq;
use crate::request::Request;
use crate::session::Input;

/// 替身记着的提前压好。
#[derive(Default)]
pub(super) struct Prepares {
    /// 回合开始时交给内核的开关（`TurnStartHooksDone` 的 `prepare`）：默认关着。
    pub(super) on: bool,
    /// 提前压的请求，照先后：照到第几条，和请求本身。
    pub(super) asked: Vec<(Seq, Request)>,
    /// 接下来几次怎么回，没排的不回。
    pub(super) lines: VecDeque<Line>,
    /// 停住的那一次：它的 `upto` 和剩下的回复。
    pub(super) held: Option<(Seq, Line)>,
}

impl Stage {
    /// 之后的回合开着、关着提前压好。
    pub fn prepare(&mut self, on: bool) {
        self.prepare.on = on;
    }

    /// 提前压的请求接下来几次，照先后这样回。没排的只记下、不回。
    pub fn prepare_model(&mut self, lines: impl IntoIterator<Item = Line>) {
        self.prepare.lines.extend(lines);
    }

    /// 提前压的请求，照先后：照到第几条，和请求本身。
    pub fn prepares(&self) -> &[(Seq, Request)] {
        &self.prepare.asked
    }

    /// 放行停住的那次提前压：送说完了。
    ///
    /// # Panics
    ///
    /// 没有停住的。
    pub fn release_prepare(&mut self) {
        let (upto, line) = self
            .prepare
            .held
            .take()
            .unwrap_or_else(|| panic!("没有停住的提前压"));
        // 只送了头几个字的（施工 6-11 下）：先送剩下的、收全。
        if line.partway.is_some() {
            for delta in partway(&line, line.partway.unwrap_or_default(), false) {
                let input = Input::AsideDelta {
                    at: self.tick(),
                    purpose: Purpose::Compaction,
                    upto,
                    delta,
                };
                self.run(input);
            }
        }
        let ended = self.aside_ended(Purpose::Compaction, upto, &line);
        self.run(ended);
    }
}

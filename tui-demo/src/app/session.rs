//! 和核心的会话打交道的几样：连不上时发不出去（蓝图 `tui.md`「连核心」第 8 条）、`/new` 开新会话、刚开还没说话时
//! 撤销、压缩、清空当场答（「斜杠命令」`/new`），切权限级别（「权限级别」第 2 条）。

use super::App;
use crate::commands::Run;
use crate::core::{Command, Update};
use crate::transcript::Link;

impl App {
    /// 发给核心的发不发得出去：连不上时（左下角是红字）不发，弹提示；正在连、正在重连的排着，连上再发。
    pub(super) fn reachable(&mut self) -> bool {
        if matches!(self.transcript.link, Link::Down(_)) {
            let note = self.config.text.not_connected.clone();
            self.hint(note, false);
            return false;
        }
        true
    }

    /// `/new`：清界面回首页；核心那边退订旧会话，第一句话时再开。旧会话在跑的那一轮照跑完。
    pub(super) fn new_session(&mut self) {
        self.core.send(Command::New);
        self.transcript.fresh();
        self.view = Default::default();
        self.panel = None;
    }

    /// Tab、Shift+Tab：切到下一档，告诉核心，等 `session.policy_changed` 来了再画（「权限级别」第 2 条）。会话还没开的
    /// 界面先照按的画，核心那边开会话时补发；连不上核心的发不出去。
    pub(super) fn cycle_level(&mut self) {
        if !self.reachable() {
            return;
        }
        let target = self.transcript.next_level(&self.config.layout.level_cycle);
        if self.not_opened() {
            self.transcript.level = target;
        }
        self.core.send(Command::Level(target));
    }

    /// 按过 `/new`、还没说话：连着核心，却还没开会话。
    pub(super) fn not_opened(&self) -> bool {
        self.transcript.session.is_none() && self.transcript.link == Link::Ready
    }

    /// 还没开的会话没有能撤销、恢复、压缩的：照核心会说的当场答，不去开一个会话。
    pub(super) fn nothing_yet(&mut self, run: Run) {
        let reason = match run {
            Run::Revert => "nothing_to_revert",
            Run::Unrevert => "nothing_to_unrevert",
            Run::Clear => "nothing_to_clear",
            Run::Redo | Run::Edit => "not_redoable",
            _ => "nothing_to_compact",
        };
        self.core(Update::Refused {
            reason: Some(reason.to_string()),
            message: String::new(),
        });
    }
}

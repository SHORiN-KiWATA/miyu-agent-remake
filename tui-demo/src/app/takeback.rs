//! Ctrl+C 打断时退回排着的话（蓝图 `tui.md`「按键」`Ctrl+C`，2026-09-30 项目主人定只退回排着的）：还排着的核心照常退回；
//! 这一轮是排着的话开的、她还没开口的，那几句已经发给模型、核心不退，打断以后等这一轮结束接着撤掉它，照排着时的样子
//! 放回输入框。

use super::{App, paste};
use crate::core::{Command, Push, Update};
use crate::input::Draft;
use crate::transcript::Chip;

/// 退回走到哪一步。
#[derive(Debug)]
pub enum Takeback {
    /// 打断发出去了，等这一轮结束：那几句。
    Interrupting(Vec<(String, Vec<Chip>)>),
    /// 撤销发出去了，等撤销成了：那几句。
    Reverting(Vec<(String, Vec<Chip>)>),
}

impl App {
    /// Ctrl+C 打断（排着的退回）：这一轮是排着的话开的、她还没开口的，记下那几句，等结束了撤掉这一轮。
    pub(super) fn interrupt(&mut self) {
        self.takeback = self.transcript.takeback().map(Takeback::Interrupting);
        self.core.send(Command::Interrupt { send: false });
    }

    /// 正在看的会话推来的：这一轮结束了发撤销；撤销成了把那几句放回输入框；撤销被拒了就算了。
    pub(super) fn takeback_on(&mut self, update: &Update) {
        match (self.takeback.take(), update) {
            (Some(Takeback::Interrupting(back)), Update::Push(Push::TurnEnded(_))) => {
                self.core.send(Command::Revert);
                self.takeback = Some(Takeback::Reverting(back));
            }
            (Some(Takeback::Reverting(back)), Update::Undone { restore: false, .. }) => {
                self.put_returned(back);
            }
            (Some(Takeback::Reverting(_)), Update::Refused { .. }) => {}
            (kept, _) => self.takeback = kept,
        }
    }

    /// 退回的话连同粘贴块放回输入框，一条之间空一行，接在已有的字前面（`tui.md`「输入框」第 8、11 条）。输入历史里
    /// 找得到的照发出去时的样子：附件跟着回来。
    pub(super) fn put_returned(&mut self, returned: Vec<(String, Vec<Chip>)>) {
        if returned.is_empty() {
            return;
        }
        let mut draft = Draft::default();
        for (text, chips) in returned {
            let sent = self.input.sent_by_text(&text);
            let back = sent.unwrap_or_else(|| Draft::from_pasted(&text, &paste::pasted(chips)));
            draft.append(back, "\n\n");
        }
        if !self.input.editor.is_empty() {
            draft.append(self.input.draft(), "\n\n");
        }
        self.input.editor.set_draft(draft);
    }
}

//! 配置页的鼠标（蓝图「配置页」第 6、15 条）：点分页换页，点一行选中、点选中的那行或双击开窗，点悬浮窗里的行、按钮。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};

use ratatui::layout::Position;

use super::forms::Focus;
use super::nav::PAGES;
use super::popup::Popup;
use super::{Hit, Outcome, Settings, Texts};

impl Settings {
    /// 左键点了 `at`；`window` 以内又点同一处的算双击。
    pub fn click(&mut self, at: Position, window: Duration, texts: &Texts) -> Outcome {
        let now = Instant::now();
        let double = self
            .last_click
            .replace((at, now))
            .is_some_and(|(was, then)| was == at && now.duration_since(then) <= window);
        let Some(hit) = self
            .hits
            .iter()
            .find(|(r, _)| r.contains(at))
            .map(|(_, h)| *h)
        else {
            return Outcome::Stay;
        };
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        match hit {
            Hit::Menu(i) => {
                self.more.menu_at = i;
                return self.key(enter, texts);
            }
            Hit::Tab(i) if self.popup.is_none() => {
                self.nav.turn(PAGES[i.min(PAGES.len() - 1)]);
                self.nav.col = 0;
            }
            Hit::Row(col, row) if self.popup.is_none() => {
                let cols = self.nav.cols(&self.view);
                let again = self.nav.focus(&self.view) == col && self.nav.selected(col) == row;
                if let Some(at) = cols.iter().position(|c| *c == col) {
                    self.nav.col = at;
                }
                self.nav.pick(col, row);
                if again || double {
                    return self.key(enter, texts);
                }
            }
            Hit::FormRow(i) => {
                if let Some(Popup::Form(form)) = &mut self.popup {
                    let again = form.focus == Focus::Row(i);
                    form.focus = Focus::Row(i);
                    if again || double {
                        return self.key(enter, texts);
                    }
                }
            }
            Hit::PickRow(i) => {
                if let Some(Popup::Pick(pick)) = &mut self.popup {
                    let again = pick.sel == i;
                    pick.sel = i;
                    if again || double {
                        return self.key(enter, texts);
                    }
                }
            }
            Hit::Button(b) => {
                match &mut self.popup {
                    Some(Popup::Form(form)) => form.focus = Focus::Button(b),
                    Some(Popup::Confirm(confirm)) => confirm.sel = b,
                    _ => return Outcome::Stay,
                }
                return self.key(enter, texts);
            }
            _ => {}
        }
        Outcome::Stay
    }
}

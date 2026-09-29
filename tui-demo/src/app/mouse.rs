//! 鼠标：按下时落在哪一块，拖动、松开都归它；点开收起时间线；命令列表的悬停和点击（`tui.md`「鼠标」）。

use ratatui::crossterm::event::{MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::{App, Grab};
use crate::body_view::BodyAction;
use crate::commands::Spec;
use crate::input::Action;
use crate::ui::rows::Target;

impl App {
    /// 鼠标：按下时看落在哪一块，之后的拖动、松开都归那一块；悬停、滚轮照位置分。
    pub(super) fn mouse(&mut self, mouse: MouseEvent) -> Action {
        let at = Position::new(mouse.column, mouse.row);
        // 首页的吉祥物没字可看时跟着鼠标转头（`tui.md`「空会话的首页」第 6 条）。
        self.pointer = Some(at);
        if self.jobs_mouse(mouse, at) {
            return Action::None;
        }
        let under = if self.areas.menu.contains(at) {
            Some(Grab::Menu)
        } else if self.areas.frame.contains(at) {
            Some(Grab::Input)
        } else if self.areas.body.contains(at) {
            Some(Grab::Body)
        } else {
            None
        };
        let owner = match mouse.kind {
            MouseEventKind::Down(_) => {
                self.grab = under;
                under
            }
            MouseEventKind::Drag(_) => self.grab,
            MouseEventKind::Up(_) => self.grab.take(),
            _ => under,
        };
        if owner != Some(Grab::Body) {
            self.view.hover = None;
        }
        match owner {
            Some(Grab::Menu) if self.history.open => self.history_mouse(mouse),
            Some(Grab::Menu) => match self.menu_matches() {
                Some(matches) => self.menu_mouse(mouse, &matches),
                None => Action::None,
            },
            Some(Grab::Input) => self.input.mouse(mouse, under == Some(Grab::Input)),
            Some(Grab::Body) => {
                match self.view.mouse(mouse) {
                    BodyAction::None => {}
                    BodyAction::Toggle(target) => self.toggle(target),
                    BodyAction::Open(url) => {
                        if let Err(e) = crate::open::open(&url) {
                            self.hint(e.to_string(), false);
                        }
                    }
                }
                Action::None
            }
            None => Action::None,
        }
    }

    /// 点开、收起时间线的一段或一步。
    pub(super) fn toggle(&mut self, target: Target) {
        let entries = &mut self.transcript.entries;
        match target {
            Target::Segment(i) => {
                if let Some(segment) = entries.get_mut(i).and_then(|e| e.segment.as_mut()) {
                    segment.open = Some(!segment.expanded(self.config.timeline.fold));
                }
            }
            Target::Entry(i) => {
                if let Some(entry) = entries.get_mut(i) {
                    entry.open = !entry.open;
                }
            }
            Target::Step(i, j) => {
                let segment = entries.get_mut(i).and_then(|e| e.segment.as_mut());
                if let Some(step) = segment.and_then(|s| s.steps.get_mut(j)) {
                    step.open = Some(!step.opened(&self.config.timeline));
                }
            }
        }
    }

    /// 输入历史列表上的鼠标：悬停选中那一条，点一下等于 `Enter`（`tui.md`「输入历史列表」第 5 条）。
    fn history_mouse(&mut self, mouse: MouseEvent) -> Action {
        use ratatui::crossterm::event::MouseButton;
        let found = self.history.matches(self.input.sent());
        let width = self.areas.menu.width;
        let rows = crate::ui::history_lines(&self.history, &found, width, &self.config);
        let at = crate::ui::history_index_at(self.areas.menu, &rows, mouse.row);
        if let Some(index) = at {
            match mouse.kind {
                MouseEventKind::Moved => self.history.selected = index,
                MouseEventKind::Down(MouseButton::Left) => {
                    self.history.selected = index;
                    self.pick_history();
                }
                _ => {}
            }
        }
        Action::None
    }

    /// 列表上的鼠标：悬停就选中，点一下就执行。
    pub(super) fn menu_mouse(&mut self, mouse: MouseEvent, matches: &[Spec]) -> Action {
        use ratatui::crossterm::event::{MouseButton, MouseEventKind};
        let rows = self.config.layout.menu_rows;
        let top = crate::menu::window(self.menu.selected, matches.len(), rows);
        let index = top + usize::from(mouse.row - self.areas.menu.y);
        let Some(spec) = matches.get(index) else {
            return Action::None;
        };
        match mouse.kind {
            MouseEventKind::Moved => self.menu.selected = index,
            MouseEventKind::Down(MouseButton::Left) => {
                let spec = spec.clone();
                self.input.editor.take();
                self.run(&spec);
            }
            _ => {}
        }
        Action::None
    }
}

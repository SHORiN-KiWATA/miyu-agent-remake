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
        // 抽屉开着：悬停选中那一项，点一下等于 `Enter`（`tui.md`「确认和提问的抽屉」第 4 条）。
        if self.drawers.open() && self.areas.text.contains(at) {
            let row = usize::from(at.y - self.areas.text.y);
            if let Some(index) = self.drawer_rows.get(row).copied().flatten() {
                match mouse.kind {
                    MouseEventKind::Moved => {
                        if let Some(d) = self.drawers.current.as_mut() {
                            d.cursor[d.tab] = index;
                        }
                    }
                    MouseEventKind::Down(_) => self.drawer_click(index),
                    _ => {}
                }
            }
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
            // 回答里的 `<details>`：点过的记在这一条上，再点一下去掉（和写的 `open` 反过来）。
            Target::Details(i, k) => {
                if let Some(entry) = entries.get_mut(i) {
                    match entry.details.iter().position(|&d| d == k) {
                        Some(at) => {
                            entry.details.remove(at);
                        }
                        None => entry.details.push(k),
                    }
                }
            }
        }
    }

    /// 输入历史列表上的鼠标：悬停选中那一条，点一下等于 `Enter`（`tui.md`「输入历史列表」第 5 条）。
    fn history_mouse(&mut self, mouse: MouseEvent) -> Action {
        use ratatui::crossterm::event::MouseButton;
        let found = self.history.matches(self.input.sent());
        let width = self.areas.menu_text.width;
        let now = std::time::Instant::now();
        // 和画出来的同一份：放进列表那块的高度（「窗口小的时候」第 1 条）。
        let max = usize::from(self.areas.menu_text.height);
        let (_, rows) =
            crate::ui::history_lines(&self.history, &found, width, &self.config, now, max);
        let at = crate::ui::history_index_at(self.areas.menu_text, &rows, mouse.row);
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
        // 露了几条照画出来的：框里放字的那一块有几行（「窗口小的时候」第 1 条）。
        let rows = crate::ui::menu_rows(self.config.layout.menu_rows, self.areas.menu_text.height);
        let at = crate::ui::menu_index_at(
            self.areas.menu_text,
            matches.len(),
            self.menu.selected,
            rows,
            mouse.row,
        );
        let Some((index, spec)) = at.and_then(|i| Some((i, matches.get(i)?))) else {
            return Action::None;
        };
        match mouse.kind {
            MouseEventKind::Moved => self.menu.selected = index,
            MouseEventKind::Down(MouseButton::Left) => {
                let spec = spec.clone();
                self.input.editor.take();
                self.run(&spec, None);
            }
            _ => {}
        }
        Action::None
    }
}

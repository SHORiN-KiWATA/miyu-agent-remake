//! `/sessions`：会话列表的框和切会话（蓝图 `tui.md`「会话列表 `/sessions`」）。打字筛，`↑` `↓` 选，`Enter` 切过去，
//! `Ctrl+P` 置顶、取消，`Ctrl+D` 按两次删，`Esc` 关；鼠标悬停选中、点一下切过去。
//!
//! 切走时原来那个还忙着的（有一轮在跑、有任务没报完）停放着、接着订阅，空下来再退订（第 4 条，和 `/new` 一个规矩）；
//! 切过去的停放着的直接换回来，没有的开一份空的正文，等核心补发以前的事件（第 5 条）。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::sessions::Parked;
use super::{App, Panel};
use crate::core::{Command, SessionInfo};
use crate::session_list::SessionList;
use crate::transcript::Transcript;

impl App {
    /// 开框，向核心要列表；交回来以前框里写「正在读」。
    pub(super) fn open_sessions(&mut self) {
        if !self.reachable() {
            return;
        }
        // 这次启动里开过的：先照上次交回的画，交回新的再换（第 2 条：核心每列一次都把日志整份读一遍，会话多了慢）。
        let mut list = SessionList::default();
        if let Some(seen) = self.sessions_seen.clone() {
            list.replace(seen);
        }
        self.session_list = Some(list);
        self.panel = Some(Panel::Sessions);
        self.core.send(Command::ListSessions);
    }

    /// 核心交回了列表。
    pub(super) fn sessions_listed(&mut self, all: Vec<SessionInfo>) {
        self.sessions_seen = Some(all.clone());
        if let Some(list) = &mut self.session_list {
            list.replace(all);
        }
    }

    /// 主会话：切进了子会话的是停放着的那个。
    pub fn main_session(&self) -> Option<String> {
        self.home
            .clone()
            .or_else(|| self.transcript.session.clone())
    }

    /// 框开着时的按键。
    pub(super) fn sessions_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Some(list) = self.session_list.as_mut() else {
            self.panel = None;
            return;
        };
        match key.code {
            KeyCode::Esc => self.close_sessions(),
            KeyCode::Up => list.step(false),
            KeyCode::Down => list.step(true),
            KeyCode::Enter => {
                if let Some(id) = list.picked().map(|s| s.session.clone()) {
                    self.switch_session(id);
                }
            }
            KeyCode::Char(' ') => {
                let main = self.main_session();
                if let Some(list) = self.session_list.as_mut() {
                    list.tick(main.as_deref());
                }
            }
            KeyCode::Char('a') if ctrl => {
                let main = self.main_session();
                if let Some(list) = self.session_list.as_mut() {
                    list.tick_all(main.as_deref());
                }
            }
            KeyCode::Char('p') if ctrl => self.toggle_pin(),
            KeyCode::Char('d') if ctrl => self.delete_picked(),
            KeyCode::Backspace => {
                let mut query = list.query.clone();
                query.pop();
                list.typed(query);
            }
            KeyCode::Char(c) if !ctrl => {
                let query = format!("{}{c}", list.query);
                list.typed(query);
            }
            _ => {}
        }
    }

    /// 鼠标在框上：悬停选中，点一下切过去；交回归不归它。
    pub(super) fn sessions_mouse(&mut self, mouse: MouseEvent, at: Position) -> bool {
        if self.panel != Some(Panel::Sessions) {
            return false;
        }
        let areas = self.areas;
        if !areas.menu.contains(at) {
            return false;
        }
        // 滚轮挪露出的那一段，选中的不动（照输入历史列表）。
        let by = match mouse.kind {
            MouseEventKind::ScrollUp => -1,
            MouseEventKind::ScrollDown => 1,
            _ => 0,
        };
        if by != 0 {
            let rows = self.config.layout.session_rows.max(1);
            if let Some(list) = self.session_list.as_mut() {
                let count = list.matches().len();
                crate::menu::pin(list.selected, &mut list.pinned, count, rows, by);
            }
            return true;
        }
        let row = at.y.checked_sub(areas.menu_text.y).map(usize::from);
        let index = row.and_then(|row| self.panel_rows.get(row).copied().flatten());
        let Some(index) = index.filter(|_| areas.menu_text.contains(at)) else {
            return true;
        };
        let Some(list) = self.session_list.as_mut() else {
            return true;
        };
        let rows = self.config.layout.session_rows.max(1);
        let count = list.matches().len();
        crate::menu::pin(list.selected, &mut list.pinned, count, rows, 0);
        list.selected = index;
        list.armed = None;
        if matches!(mouse.kind, MouseEventKind::Down(_))
            && let Some(id) = list.picked().map(|s| s.session.clone())
        {
            self.switch_session(id);
        }
        true
    }

    fn close_sessions(&mut self) {
        self.panel = None;
        // 框里改过的（置顶、删了的）记下来，下次开框先照它画。
        if let Some(list) = self.session_list.take().filter(|l| l.loaded) {
            self.sessions_seen = Some(list.all);
        }
    }

    /// 置顶、取消选中的那个：框里当场换，再向核心要一份新的。
    fn toggle_pin(&mut self) {
        let Some(list) = self.session_list.as_mut() else {
            return;
        };
        let Some(session) = list.picked().map(|s| s.session.clone()) else {
            return;
        };
        let mut all = std::mem::take(&mut list.all);
        let mut pinned = false;
        for info in all.iter_mut().filter(|s| s.session == session) {
            info.pinned = !info.pinned;
            pinned = info.pinned;
        }
        list.replace(all);
        // 框里当场改，不向核心重读整份列表（慢，读回来还会跳，第 2 条）。
        self.core.send(Command::Pin { session, pinned });
        let texts = &self.config.text.sessions;
        let note = if pinned {
            &texts.pinned
        } else {
            &texts.unpinned
        };
        self.hint(note.clone(), true);
    }

    /// `Ctrl+D`：删勾了的全部、没勾的删选中的那一个；整批按第一次提示再按一次，第二次才删；正在用的不能删（第 2 条）。
    fn delete_picked(&mut self) {
        let main = self.main_session();
        let texts = self.config.text.sessions.clone();
        let untitled = self.config.text.untitled.clone();
        let Some(list) = self.session_list.as_mut() else {
            return;
        };
        let doomed = list.doomed(main.as_deref());
        let title = |list: &SessionList, id: &str| {
            let found = list.all.iter().find(|s| s.session == id);
            found
                .and_then(|s| s.title.clone())
                .unwrap_or(untitled.clone())
        };
        let note = match doomed.as_slice() {
            [] => {
                self.hint(texts.delete_current, false);
                return;
            }
            [one] => title(list, one),
            many => many.len().to_string(),
        };
        let one = doomed.len() == 1;
        if list.armed.as_ref() != Some(&doomed) {
            list.armed = Some(doomed);
            let ask = if one {
                texts.delete_again.replace("{title}", &note)
            } else {
                texts.delete_many.replace("{count}", &note)
            };
            self.hint(ask, false);
            return;
        }
        list.remove(&doomed);
        for session in doomed {
            // 切走时还忙着、停放着的：不再订阅。
            if let Some(parked) = self.parked.remove(&session) {
                self.left.retain(|s| *s != session);
                self.drop_children(&parked.board);
                self.core.send(Command::Unwatch(session.clone()));
            }
            self.core.send(Command::Delete(session));
        }
        let done = if one {
            texts.deleted.replace("{title}", &note)
        } else {
            texts.deleted_many.replace("{count}", &note)
        };
        self.hint(done, true);
    }

    /// 这个会话还忙着：有一轮在跑，或者还有任务没报完（第 4 条）。
    pub(super) fn busy_here(&self) -> bool {
        self.board.busy() || self.transcript.running.is_some()
    }

    /// 切到 `session`（第 4、5 条）：停放着的（切走时还忙着的）直接换上来；别的先开一份停放着的空正文收补发来的，
    /// 补完了再换上来，这之间照旧显示原来那个（不闪空会话的首页）。
    pub(super) fn switch_session(&mut self, session: String) {
        self.close_sessions();
        self.leave_child();
        if self.transcript.session.as_deref() == Some(session.as_str()) {
            return;
        }
        let keep = self.busy_here();
        self.core.send(Command::Open {
            session: session.clone(),
            keep,
        });
        if !self.parked.contains_key(&session) {
            let mut fresh = Transcript::default();
            fresh.session = Some(session.clone());
            fresh.link = self.transcript.link.clone();
            fresh.level = self.transcript.level;
            let parked = Parked {
                transcript: fresh,
                ..Parked::default()
            };
            self.parked.insert(session.clone(), parked);
            self.opening = Some((session, keep));
            return;
        }
        self.swap_in(&session, keep);
    }

    /// 补发完了（或者补不成）：把在补发的那个换上来。
    pub(super) fn opened(&mut self) {
        if let Some((session, keep)) = self.opening.take() {
            self.swap_in(&session, keep);
        }
    }

    /// 在补发的是不是 `session`。
    pub(super) fn opening(&self, session: &str) -> bool {
        self.opening.as_ref().is_some_and(|(s, _)| s == session)
    }

    /// 换上停放着的 `session`，原来那个照 `keep` 停放或者不要了。
    fn swap_in(&mut self, session: &str, keep: bool) {
        self.park_current(keep);
        if let Some(back) = self.parked.remove(session) {
            self.left.retain(|s| s != session);
            let link = self.transcript.link.clone();
            self.transcript = back.transcript;
            self.transcript.link = link;
            self.board = back.board;
            self.view = back.view;
        }
        self.prune();
    }
}

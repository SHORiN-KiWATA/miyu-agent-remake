//! `/model`：换这个会话的模型（蓝图 `tui.md`「配置与模型」第 1 条，核心 8-10）。开框时向核心要一次 `model.list`；打字筛，
//! `↑` `↓` 选，`Enter` 交给核心换（下一个回合开始生效），冷却中的、没有 key 的不换、提示为什么；`Esc` 关。鼠标悬停
//! 选中、点一下等于 `Enter`，滚轮滚（照会话列表）。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::{App, Panel};
use crate::core::{Choice, ChoiceState, Command};
use crate::model_list::ModelList;

impl App {
    /// 开框，向核心要一行行。
    pub(super) fn open_models(&mut self) {
        if !self.reachable() {
            return;
        }
        self.model_list = Some(ModelList::default());
        self.panel = Some(Panel::Models);
        self.core.send(Command::ListChoices);
    }

    /// 核心交回了一行行。
    pub(super) fn models_listed(&mut self, all: Vec<Choice>) {
        let current = self.transcript.model_ref().map(str::to_string);
        if let Some(list) = &mut self.model_list {
            list.replace(all, current.as_deref());
        }
    }

    /// 框开着时的按键。
    pub(super) fn models_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Some(list) = self.model_list.as_mut() else {
            self.panel = None;
            return;
        };
        match key.code {
            KeyCode::Esc => self.close_models(),
            KeyCode::Up => list.step(false),
            KeyCode::Down => list.step(true),
            KeyCode::Enter => {
                if let Some(choice) = list.picked().cloned() {
                    self.choose_model(&choice);
                }
            }
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

    /// 鼠标在框上：悬停选中，点一下换，滚轮挪露出的那一段；交回归不归它。
    pub(super) fn models_mouse(&mut self, mouse: MouseEvent, at: Position) -> bool {
        if self.panel != Some(Panel::Models) || !self.areas.menu.contains(at) {
            return false;
        }
        let rows = self.config.layout.session_rows.max(1);
        let by = match mouse.kind {
            MouseEventKind::ScrollUp => -1,
            MouseEventKind::ScrollDown => 1,
            _ => 0,
        };
        let row = at.y.checked_sub(self.areas.menu_text.y).map(usize::from);
        let index = row.and_then(|row| self.panel_rows.get(row).copied().flatten());
        let inside = self.areas.menu_text.contains(at);
        let Some(list) = self.model_list.as_mut() else {
            return true;
        };
        let count = list.matches().len();
        if by != 0 {
            crate::menu::pin(list.selected, &mut list.pinned, count, rows, by);
            return true;
        }
        let Some(index) = index.filter(|_| inside) else {
            return true;
        };
        crate::menu::pin(list.selected, &mut list.pinned, count, rows, 0);
        list.selected = index;
        if matches!(mouse.kind, MouseEventKind::Down(_))
            && let Some(choice) = list.picked().cloned()
        {
            self.choose_model(&choice);
        }
        true
    }

    fn close_models(&mut self) {
        self.panel = None;
        self.model_list = None;
    }

    /// 换成选中的那个：冷却中的、没有 key 的不换，提示为什么；别的交给核心，关框。
    fn choose_model(&mut self, choice: &Choice) {
        let texts = self.config.text.model_panel.clone();
        match choice.state {
            ChoiceState::Cooling(until) => {
                let left = crate::ui::model_list::cooling(until, jiff::Timestamp::now(), &texts);
                let minutes: String = left.chars().filter(char::is_ascii_digit).collect();
                let note = if minutes.is_empty() {
                    left
                } else {
                    texts.refuse_cooling.replace("{n}", &minutes)
                };
                self.hint(note, false);
            }
            ChoiceState::NoKey => self.hint(texts.refuse_no_key, false),
            ChoiceState::Ok => {
                self.close_models();
                if self.transcript.model_ref() == Some(choice.reference.as_str()) {
                    return;
                }
                // 不弹提示：底栏当场就写成选的那个（2026-10-01 项目主人：改模型、思考强度不要通知）。
                self.core.send(Command::Configure(choice.reference.clone()));
            }
        }
    }
}

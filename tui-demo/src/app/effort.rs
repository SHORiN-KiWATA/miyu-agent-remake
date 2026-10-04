//! `/effort`：换正在用的模型的思考强度（蓝图 `tui.md`「配置与模型」思考强度，核心 8-18 补；2026-10-02 项目主人定：只要
//! 这个框）。开框时向核心要一次 `model.list`，照正在请求的模型（没有的照会话的引用、默认的聊天模型）列它的几级；
//! `Enter` 写进个人设置（默认的去掉这一项），关框，不弹提示，所有会话下一轮起照它，底栏当场变。用的是模型池的提示一句，
//! 不开框（2026-10-02 项目主人定）。

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::{App, Panel};
use crate::core::{Command, EffortList, Efforts};

impl App {
    /// 开框，向核心要几级。
    pub(super) fn open_effort(&mut self) {
        if !self.reachable() {
            return;
        }
        self.efforts = None;
        self.panel = Some(Panel::Effort { selected: 0 });
        self.core.send(Command::ListEfforts);
    }

    /// 向核心要一次 `model.list`，底栏照它写模型和这一档：连上核心、`/new` 以后、换了强度以后（2026-10-02 项目主人
    /// 报：空会话的底栏没有模型、强度，换了强度要等下一轮才变）。
    pub(super) fn refresh_effort(&mut self) {
        self.core.send(Command::ListEfforts);
    }

    /// 核心交回了：底栏照它写；框开着的，找出改哪个模型、选着配置的默认那一级，用的是模型池的关框、提示一句。
    pub(super) fn efforts_listed(&mut self, list: EffortList) {
        self.footer_effort(&list);
        if !matches!(self.panel, Some(Panel::Effort { .. })) {
            return;
        }
        let Some(target) = self.effort_target(&list).cloned() else {
            self.panel = None;
            let note = self.config.text.effort.need_model.clone();
            self.hint(note, false);
            return;
        };
        let selected = crate::ui::effort_list::initial(&target);
        self.panel = Some(Panel::Effort { selected });
        self.efforts = Some(target);
    }

    /// 底栏：还没开会话的照默认的聊天模型写模型和这一档；开了的、改的就是正在用的那个，照新的最终值写这一档。
    fn footer_effort(&mut self, list: &EffortList) {
        let Some(target) = self.effort_target(list).cloned() else {
            return;
        };
        if self.transcript.model_ref().is_none() {
            self.transcript
                .show_default(&target.reference, target.current.clone());
            return;
        }
        let in_use = self
            .transcript
            .model
            .as_ref()
            .map(|(m, e)| format!("{e}/{m}"));
        if in_use.as_deref() == Some(target.reference.as_str()) {
            self.transcript.set_effort(target.current);
        }
    }

    /// 改哪个模型：正在请求的那个，没有的照会话的引用，还没开会话的照默认的聊天模型。
    fn effort_target<'a>(&self, list: &'a EffortList) -> Option<&'a Efforts> {
        let in_use = self
            .transcript
            .model
            .as_ref()
            .map(|(m, e)| format!("{e}/{m}"));
        list.target(in_use.as_deref(), self.transcript.model_ref())
    }

    /// 框开着时的按键。
    pub(super) fn effort_key(&mut self, selected: usize, key: KeyEvent) {
        let last = self.efforts.as_ref().map_or(0, |t| t.levels.len());
        let at = |selected| Some(Panel::Effort { selected });
        match key.code {
            KeyCode::Up => self.panel = at(selected.saturating_sub(1)),
            KeyCode::Down => self.panel = at((selected + 1).min(last)),
            KeyCode::Enter => self.choose_effort(selected),
            KeyCode::Esc => self.panel = None,
            _ => {}
        }
    }

    /// 鼠标在框上：悬停选中，点一下换；交回归不归它。
    pub(super) fn effort_mouse(&mut self, mouse: MouseEvent, at: Position) -> bool {
        let Some(Panel::Effort { .. }) = self.panel else {
            return false;
        };
        let areas = self.areas;
        if !areas.menu.contains(at) {
            return false;
        }
        let row = at.y.checked_sub(areas.menu_text.y).map(usize::from);
        let index = row.and_then(|row| self.panel_rows.get(row).copied().flatten());
        if let Some(index) = index.filter(|_| areas.menu_text.contains(at)) {
            self.panel = Some(Panel::Effort { selected: index });
            if matches!(mouse.kind, MouseEventKind::Down(_)) {
                self.choose_effort(index);
            }
        }
        true
    }

    /// 换成第 `index` 行（第 0 行默认）：写个人设置，关框，不弹提示（2026-10-01 项目主人：改模型、思考强度不要通知）。
    /// 和配置的默认一样的只关框。
    fn choose_effort(&mut self, index: usize) {
        self.panel = None;
        let Some(target) = self.efforts.take() else {
            return;
        };
        let level = index
            .checked_sub(1)
            .and_then(|i| target.levels.get(i).cloned());
        if index > 0 && level.is_none() || level == target.current {
            return;
        }
        self.core.send(Command::SetEffort {
            key: target.key,
            level,
        });
        // 写完马上再要一次，底栏当场照新的最终值写（个人的去掉了，系统配置里可能还有一档）。
        self.refresh_effort();
    }
}

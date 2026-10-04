//! 列表开着时 `Ctrl+J`、`Ctrl+K` 当 `↓`、`↑`（蓝图 `tui.md`「按键」，2026-10-01 项目主人要 vim 的上下）。
//! 只换按键本身，后面照常分给开着的那个列表；抽屉里写字时 `Ctrl+J` 照旧换行，不换。

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use super::App;

impl App {
    /// 有列表、面板开着，或者抽屉开着、没在写字时，换掉这个按键；`menu_open`：命令列表、`@` 文件列表开着。
    pub(super) fn vim_keys(&self, event: Event, menu_open: bool) -> Event {
        let listing = menu_open
            || self.history.open
            || self.panel.is_some()
            || (self.drawers.open() && !self.drawers.editing());
        match event {
            Event::Key(key) => Event::Key(step(key, listing)),
            other => other,
        }
    }
}

/// `list_open`：有列表、面板开着，或者抽屉开着、没在写字。是的话把 `Ctrl+J`、`Ctrl+K` 换成 `↓`、`↑`。
pub fn step(key: KeyEvent, list_open: bool) -> KeyEvent {
    if !list_open || key.modifiers != KeyModifiers::CONTROL {
        return key;
    }
    let code = match key.code {
        KeyCode::Char('j' | 'J') => KeyCode::Down,
        KeyCode::Char('k' | 'K') => KeyCode::Up,
        _ => return key,
    };
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        ..key
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::step;

    #[test]
    fn ctrl_j_and_k_move_only_while_a_list_is_open() {
        let ctrl = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL);
        assert_eq!(step(ctrl('j'), true).code, KeyCode::Down);
        assert_eq!(step(ctrl('k'), true).code, KeyCode::Up);
        assert_eq!(step(ctrl('k'), true).modifiers, KeyModifiers::NONE);
        assert_eq!(step(ctrl('j'), false), ctrl('j'), "没开列表：照旧换行");
        let plain = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(step(plain, true), plain, "不按 Ctrl 的照常打字");
        let shifted = KeyEvent::new(
            KeyCode::Char('J'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(step(shifted, true), shifted);
    }
}

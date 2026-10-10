//! 列表开着时 `Ctrl+J`、`Ctrl+K` 当 `↓`、`↑`（蓝图 `tui.md`「按键」，2026-10-01 项目主人要 vim 的上下）；开着时不能打字的框
//! 直接 `j`、`k` 也行（2026-10-11 项目主人）。
//! 只换按键本身，后面照常分给开着的那个列表；抽屉里写字时 `Ctrl+J` 照旧换行，不换。

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use super::{App, Panel};

impl App {
    /// 有列表、面板开着，或者抽屉开着、没在写字时，换掉这个按键；`menu_open`：命令列表、`@` 文件列表开着。
    pub(super) fn vim_keys(&self, event: Event, menu_open: bool) -> Event {
        let listing = menu_open
            || self.history.open
            || self.panel.is_some()
            || (self.drawers.open() && !self.drawers.editing());
        // 开着时不能打字的框；抽屉开着时按键先归抽屉（它自己认 vim 键）。
        let no_typing = !self.drawers.open()
            && matches!(
                self.panel,
                Some(
                    Panel::Pick { .. }
                        | Panel::Language { .. }
                        | Panel::Effort { .. }
                        | Panel::Background { .. }
                        | Panel::Help { .. }
                        | Panel::Usage { .. }
                )
            );
        match event {
            Event::Key(key) => Event::Key(plain(step(key, listing), no_typing)),
            other => other,
        }
    }
}

/// 开着时不能打字的框（`no_typing`：人格框、预设框、`/language`、`/effort`、后台面板、`/help`、`/usage`）：直接按
/// `j`、`k` 也换成 `↓`、`↑`（2026-10-11 项目主人）。会话列表、换模型、命令列表、输入历史打字是搜，不换。
pub fn plain(key: KeyEvent, no_typing: bool) -> KeyEvent {
    if !no_typing || key.modifiers != KeyModifiers::NONE {
        return key;
    }
    let code = match key.code {
        KeyCode::Char('j') => KeyCode::Down,
        KeyCode::Char('k') => KeyCode::Up,
        _ => return key,
    };
    KeyEvent { code, ..key }
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

    #[test]
    fn plain_j_and_k_move_in_boxes_that_take_no_typing() {
        // 2026-10-11 项目主人：人格框这类开着不能打字的框，直接 j、k 上下。
        let plain = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        assert_eq!(super::plain(plain('j'), true).code, KeyCode::Down);
        assert_eq!(super::plain(plain('k'), true).code, KeyCode::Up);
        assert_eq!(
            super::plain(plain('j'), false),
            plain('j'),
            "能打字的列表照常打字"
        );
        assert_eq!(super::plain(plain('x'), true), plain('x'));
    }
}

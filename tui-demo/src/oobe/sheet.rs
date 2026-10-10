//! 大编辑浮窗（蓝图 `tui.md`「第一次打开的引导」第 21a 条，2026-10-09 项目主人：「编辑人设的部分不用 editor 的话似乎
//! 也会导致没法编辑复杂的提示词」）：写长文的一格，`Enter` 换行，上下键照折好的行挪，`Esc` 收起，`Ctrl+G` 交给外部
//! 编辑器。画在 `ui/oobe/popup.rs`；画的时候记下折行的宽和滚到哪。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::field::Field;
use super::wrap::{offset_at, wrap};

/// 按了一个键以后要做的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetKey {
    /// 收起：写的留着。
    Close,
    /// 交给外部编辑器。
    External,
    /// 别的（打字、挪光标）。
    Stay,
}

/// 一格长文。
#[derive(Debug)]
pub struct Sheet {
    /// 字和光标。
    pub field: Field,
    /// 上一次画的时候折行的宽；还没画过的是 0（上下键退回到行首、行尾）。
    pub width: usize,
    /// 上一次画的时候从第几行露起。
    pub top: usize,
    /// 上下挪时想停的那一列：连按几次上下，短行过去了回到原来那一列。
    goal: Option<usize>,
}

impl Sheet {
    /// 照 `text` 开，光标在最后。
    pub fn open(text: &str) -> Self {
        Self {
            field: Field::lines().with(text),
            width: 0,
            top: 0,
            goal: None,
        }
    }

    /// 里面的字。
    pub fn text(&self) -> &str {
        self.field.text()
    }

    /// 换成编辑器写回来的字，光标在最后。
    pub fn replace(&mut self, text: &str) {
        self.field = Field::lines().with(text);
        self.goal = None;
    }

    /// 按了一个键。
    pub fn key(&mut self, key: KeyEvent) -> SheetKey {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => return SheetKey::Close,
            KeyCode::Char('g') if ctrl => return SheetKey::External,
            KeyCode::Up => return self.vertical(-1),
            KeyCode::Down => return self.vertical(1),
            KeyCode::Enter => self.field.editor.insert("\n"),
            _ => {
                self.field.key(key);
            }
        }
        self.goal = None;
        SheetKey::Stay
    }

    /// 粘贴：留着换行。
    pub fn paste(&mut self, text: &str) {
        self.field.paste(text);
        self.goal = None;
    }

    /// 照折好的行往上（`delta` 是 -1）、往下挪一行；第一行再往上到开头，最后一行再往下到末尾。
    fn vertical(&mut self, delta: isize) -> SheetKey {
        let text = self.field.text();
        let width = self.width.max(1);
        let (_, (row, col)) = wrap(text, self.field.editor.cursor(), width);
        let goal = *self.goal.get_or_insert(col);
        let pos = match row.checked_add_signed(delta) {
            Some(target) => offset_at(text, width, target, goal),
            None => Some(0),
        }
        .unwrap_or(text.len());
        self.field.editor.move_to(pos, false);
        SheetKey::Stay
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{Sheet, SheetKey};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn enter_breaks_a_line_and_arrows_follow_the_wrapped_rows() {
        let mut sheet = Sheet::open("abcdef");
        sheet.width = 4;
        // "abcd" / "ef"：光标在最后（第 1 行第 2 列），往上到第 0 行第 2 列。
        assert_eq!(sheet.key(key(KeyCode::Up)), SheetKey::Stay);
        assert_eq!(sheet.field.editor.cursor(), 2);
        sheet.key(key(KeyCode::Up));
        assert_eq!(sheet.field.editor.cursor(), 0, "第一行再往上到开头");
        sheet.key(key(KeyCode::Down));
        assert_eq!(sheet.field.editor.cursor(), 6, "回到原来那一列：短行的末尾");
        sheet.field.editor.move_to(4, false);
        sheet.key(key(KeyCode::Enter));
        assert_eq!(sheet.text(), "abcd\nef", "Enter 换行，不收起");
        assert_eq!(sheet.key(key(KeyCode::Esc)), SheetKey::Close);
        let ctrl_g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL);
        assert_eq!(sheet.key(ctrl_g), SheetKey::External);
    }
}

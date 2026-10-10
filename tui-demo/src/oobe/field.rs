//! 引导里打字的一格（「第一次打开的引导」第 7、17、18、21 条）：一行的、几行的、不显示的（密钥）。`Enter`、`Esc`、
//! 上下这些交回去由那一步管；粘贴照收，一行的去掉换行。光标停上去不能直接打字：`Enter` 开始编辑，编辑时 `Enter` 写好、
//! `Esc` 回到开始编辑以前的字（2026-10-10 项目主人：「回车编辑回车退出编辑才对」）。

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::Editor;

/// 一格。
#[derive(Debug, Default)]
pub struct Field {
    /// 字和光标。
    pub editor: Editor,
    /// 不显示（密钥）：画成 `•`。
    pub secret: bool,
    /// 几行的：`Shift+Enter`、`Ctrl+J` 换行，粘贴留着换行。
    pub multiline: bool,
    /// 正在编辑：开始编辑时的字（`Esc` 回到它）；没在编辑的是 `None`。
    editing: Option<String>,
    /// 网址：没在编辑时画成终端认不出的网址（[`unlink`]），不给点。
    pub unlinked: bool,
}

/// 网址画成终端认不出的样子：`://` 前面夹一个零宽空格（不占格子、看不出来）。终端照 `https://` 认网址、加下划线给点，
/// 地址那一格不该给点（2026-10-10 项目主人：「地址栏里的不应该允许点击」）。只用于画，存的、发的照原样。
pub fn unlink(text: &str) -> String {
    text.replace("://", "\u{200B}://")
}

/// 编辑时按了一个键以后。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edit {
    /// 改了字、挪了光标，或者不认的键。
    Stay,
    /// `Enter`：写好了。
    Done,
    /// `Esc`：不改了，回到开始编辑以前的字。
    Cancelled,
}

impl Field {
    /// 一行的空格子。
    pub fn line() -> Self {
        Self::default()
    }

    /// 不显示的一格（密钥）。
    pub fn secret() -> Self {
        Self {
            secret: true,
            ..Self::default()
        }
    }

    /// 几行的空格子。
    pub fn lines() -> Self {
        Self {
            multiline: true,
            ..Self::default()
        }
    }

    /// 换成 `text`，光标在最后。
    #[must_use]
    pub fn with(mut self, text: &str) -> Self {
        self.editor.set(text);
        self
    }

    /// 一直在编辑的一格（两格窗那种，进了窗就是在写）：画的时候照编辑中画、带光标。
    #[must_use]
    pub fn live(mut self) -> Self {
        self.editing = Some(String::new());
        self
    }

    /// 地址这种：没在编辑时不让终端认成网址。
    #[must_use]
    pub fn url() -> Self {
        Self {
            unlinked: true,
            ..Self::default()
        }
    }

    /// 正在编辑。
    pub fn editing(&self) -> bool {
        self.editing.is_some()
    }

    /// 开始编辑：记下现在的字，光标放到最后。
    pub fn begin(&mut self) {
        self.editing = Some(self.text().to_string());
        let end = self.text().len();
        self.editor.move_to(end, false);
    }

    /// 编辑时按了一个键：`Enter` 写好，`Esc` 回到开始编辑以前的字，别的照 [`Field::key`] 改字。
    pub fn edit(&mut self, key: KeyEvent) -> Edit {
        match key.code {
            KeyCode::Enter if !key.modifiers.contains(KeyModifiers::SHIFT) || !self.multiline => {
                self.editing = None;
                Edit::Done
            }
            KeyCode::Esc => {
                if let Some(before) = self.editing.take() {
                    self.editor.set(&before);
                }
                Edit::Cancelled
            }
            _ => {
                self.key(key);
                Edit::Stay
            }
        }
    }

    /// 里面的字。
    pub fn text(&self) -> &str {
        self.editor.text()
    }

    /// 去掉前后空白的字。
    pub fn trimmed(&self) -> String {
        self.text().trim().to_string()
    }

    /// 按了一个键：是改字的就改，交回 `true`；别的（`Enter`、`Esc`、上下、`Tab`）交回 `false`。
    pub fn key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let editor = &mut self.editor;
        match key.code {
            KeyCode::Enter if self.multiline && key.modifiers.contains(KeyModifiers::SHIFT) => {
                editor.insert("\n");
            }
            KeyCode::Char('j') if self.multiline && ctrl => editor.insert("\n"),
            KeyCode::Backspace if ctrl || alt => editor.delete_word(),
            KeyCode::Char('w') if ctrl => editor.delete_word(),
            KeyCode::Backspace => editor.backspace(),
            KeyCode::Delete => editor.delete(),
            KeyCode::Left if ctrl || alt => editor.word_left(false),
            KeyCode::Right if ctrl || alt => editor.word_right(false),
            KeyCode::Left => editor.left(false),
            KeyCode::Right => editor.right(false),
            KeyCode::Home => editor.move_to(0, false),
            KeyCode::End => editor.move_to(editor.text().len(), false),
            KeyCode::Char('a') if ctrl => editor.move_to(0, false),
            KeyCode::Char('e') if ctrl => editor.move_to(editor.text().len(), false),
            KeyCode::Char('u') if ctrl => editor.set(""),
            KeyCode::Char(c) if !ctrl && !alt => editor.insert(&c.to_string()),
            _ => return false,
        }
        true
    }

    /// 粘贴：一行的把换行换成空格（贴 key 最常见，末尾的换行去掉）。
    pub fn paste(&mut self, text: &str) {
        if self.multiline {
            self.editor.insert(text);
        } else {
            let flat = text
                .trim_end_matches(['\r', '\n'])
                .replace(['\r', '\n'], " ");
            self.editor.insert(&flat);
        }
    }

    /// 画出来的字：密钥照字数画成 `•`。
    pub fn shown(&self) -> String {
        if self.secret {
            "•".repeat(self.text().chars().count())
        } else {
            self.text().to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::Field;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn typing_editing_and_what_is_left_to_the_step() {
        let mut field = Field::line();
        for c in "ab".chars() {
            assert!(field.key(key(KeyCode::Char(c), KeyModifiers::NONE)));
        }
        assert!(field.key(key(KeyCode::Backspace, KeyModifiers::NONE)));
        assert_eq!(field.text(), "a");
        assert!(
            !field.key(key(KeyCode::Enter, KeyModifiers::NONE)),
            "回车交给那一步"
        );
        assert!(
            !field.key(key(KeyCode::Enter, KeyModifiers::SHIFT)),
            "一行的不换行"
        );
        assert!(!field.key(key(KeyCode::Up, KeyModifiers::NONE)));
        let mut many = Field::lines();
        many.key(key(KeyCode::Char('x'), KeyModifiers::NONE));
        assert!(many.key(key(KeyCode::Enter, KeyModifiers::SHIFT)));
        assert!(many.key(key(KeyCode::Char('j'), KeyModifiers::CONTROL)));
        assert_eq!(many.text(), "x\n\n");
    }

    #[test]
    fn enter_starts_and_ends_editing_and_escape_puts_the_text_back() {
        // 2026-10-10 项目主人：「回车编辑回车退出编辑才对」。
        use super::Edit;
        let mut field = Field::line().with("ab");
        assert!(!field.editing());
        field.begin();
        assert!(field.editing());
        assert_eq!(
            field.edit(key(KeyCode::Char('c'), KeyModifiers::NONE)),
            Edit::Stay
        );
        assert_eq!(
            field.edit(key(KeyCode::Esc, KeyModifiers::NONE)),
            Edit::Cancelled
        );
        assert_eq!(field.text(), "ab", "Esc 回到开始编辑以前");
        assert!(!field.editing());
        field.begin();
        field.edit(key(KeyCode::Char('c'), KeyModifiers::NONE));
        assert_eq!(
            field.edit(key(KeyCode::Enter, KeyModifiers::NONE)),
            Edit::Done
        );
        assert_eq!(field.text(), "abc");
        assert!(!field.editing());
    }

    #[test]
    fn a_pasted_key_loses_its_newline_and_is_drawn_as_dots() {
        let mut field = Field::secret();
        field.paste("sk-abc\n");
        assert_eq!(field.text(), "sk-abc");
        assert_eq!(field.shown(), "••••••");
        let mut many = Field::lines();
        many.paste("一\n二");
        assert_eq!(many.text(), "一\n二", "几行的留着换行");
    }
}

//! 输入框：文字编辑、折行、滚动、键盘和鼠标。
//!
//! 画在哪、多大由 `ui` 定；这里只记着上一次画的位置，好把鼠标坐标换回文字下标。

mod editor;
mod wrap;

#[cfg(test)]
mod tests;

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::{Position, Rect};

pub use editor::Editor;
pub use wrap::{VisualLine, locate, offset_at, pieces, tail_pieces, wrap};

/// 输入框处理完一个事件后，要外面做的事。
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// 什么都不用做。
    None,
    /// 发出去这段话。
    Submit(String),
    /// 把这段字放进剪贴板。
    Copy(String),
    /// 退出程序。
    Quit,
    /// 空着按了 `Ctrl+C`：不退出，提示用 `Ctrl+D` 退出。
    ExitHint,
}

/// 输入框的全部状态。
#[derive(Debug)]
pub struct InputBox {
    /// 最多长到几行，再多就在框里滚。
    max_rows: u16,
    /// 两次点击隔多久以内算双击。
    double_click: Duration,
    /// 文字和光标。
    pub editor: Editor,
    /// 框里第一行显示的是第几行。
    scroll: usize,
    /// 上下移动时想回到的列：经过短行时列会变小，再到长行要回到原来的列。
    goal_col: Option<usize>,
    /// 按着左键在拖。
    dragging: bool,
    /// 上一次按下左键的时间和位置，判双击。
    last_click: Option<(Instant, usize)>,
    /// 上一次画的时候，文字区在屏幕上的位置。
    area: Rect,
    /// `Ctrl+S` 暂存的字。
    stash: Option<String>,
    /// 发过的话和命令，从旧到新：翻历史用。
    history: Vec<String>,
    /// 正在翻历史，翻到第几条；没在翻是 `None`。
    browsing: Option<usize>,
    /// 开始翻历史之前没发的那句：翻过最新一条回到它。
    draft: String,
    /// 画的时候把光标滚进可见范围。滚轮滚过以后关掉，不然滚一下就被拽回光标那里。
    follow: bool,
    /// 撤销时放回来的那句：恢复时它还没动过的话收回去。
    put_back: Option<String>,
}

impl InputBox {
    /// 空的输入框。
    pub fn new(max_rows: u16, double_click: Duration) -> Self {
        Self {
            max_rows: max_rows.max(1),
            double_click,
            editor: Editor::default(),
            scroll: 0,
            goal_col: None,
            dragging: false,
            last_click: None,
            area: Rect::default(),
            stash: None,
            history: Vec::new(),
            browsing: None,
            draft: String::new(),
            follow: false,
            put_back: None,
        }
    }

    /// 按当前宽度折好的行。
    pub fn lines(&self, width: u16) -> Vec<VisualLine> {
        wrap(self.editor.text(), width)
    }

    /// 框里要露出几行：跟着文字长，最少一行，最多到配置的上限。
    pub fn rows(&self, width: u16) -> u16 {
        let n = self.lines(width).len();
        u16::try_from(n)
            .unwrap_or(self.max_rows)
            .clamp(1, self.max_rows)
    }

    /// 画之前调用：记下文字区的位置，把光标滚进可见范围。返回第一行显示第几行。
    pub fn place(&mut self, area: Rect) -> usize {
        self.area = area;
        let lines = self.lines(area.width);
        let (row, _) = locate(self.editor.text(), &lines, self.editor.cursor());
        let rows = usize::from(area.height.max(1));
        if self.follow {
            if row < self.scroll {
                self.scroll = row;
            } else if row >= self.scroll + rows {
                self.scroll = row + 1 - rows;
            }
        }
        self.scroll = self.scroll.min(lines.len().saturating_sub(rows));
        self.scroll
    }

    /// 光标在屏幕上的位置；滚出框外时是 `None`。
    pub fn cursor_position(&self) -> Option<Position> {
        let lines = self.lines(self.area.width);
        let (row, col) = locate(self.editor.text(), &lines, self.editor.cursor());
        let row = row.checked_sub(self.scroll)?;
        if row >= usize::from(self.area.height) {
            return None;
        }
        Some(Position {
            x: self.area.x + u16::try_from(col).ok()?,
            y: self.area.y + u16::try_from(row).ok()?,
        })
    }

    /// 处理一个按键。
    pub fn key(&mut self, key: KeyEvent) -> Action {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        self.follow = true;
        if !matches!(key.code, KeyCode::Up | KeyCode::Down) {
            self.goal_col = None;
        }
        match key.code {
            // Shift+Enter 要终端报得出来（kitty 键盘协议）；报不出来的终端用 Alt+Enter 或 Ctrl+J。
            KeyCode::Enter if shift || alt => self.editor.insert("\n"),
            KeyCode::Char('j') if ctrl => self.editor.insert("\n"),
            KeyCode::Enter => {
                if self.editor.text().trim().is_empty() {
                    return Action::None;
                }
                self.scroll = 0;
                self.browsing = None;
                let text = self.editor.take();
                return Action::Submit(text);
            }
            // Ctrl+Shift+C 在认得 kitty 键盘协议的终端里报成大写的 C。
            KeyCode::Char('c' | 'C') if ctrl => return self.ctrl_c(),
            KeyCode::Char('d') if ctrl && self.editor.is_empty() => return Action::Quit,
            KeyCode::Char('a') if ctrl => self.editor.select_all(),
            KeyCode::Char('s') if ctrl => self.swap_stash(),
            KeyCode::Char('w') if ctrl => self.editor.delete_word(),
            KeyCode::Backspace if ctrl || alt => self.editor.delete_word(),
            KeyCode::Left if ctrl => self.editor.word_left(shift),
            KeyCode::Right if ctrl => self.editor.word_right(shift),
            KeyCode::Char(c) if !ctrl => self.editor.insert(c.encode_utf8(&mut [0; 4])),
            KeyCode::Backspace => self.editor.backspace(),
            KeyCode::Delete => self.editor.delete(),
            KeyCode::Left => self.editor.left(shift),
            KeyCode::Right => self.editor.right(shift),
            KeyCode::Up => self.vertical(-1, shift),
            KeyCode::Down => self.vertical(1, shift),
            KeyCode::Home => self.line_edge(false, shift),
            KeyCode::End => self.line_edge(true, shift),
            KeyCode::Esc => self.editor.clear_selection(),
            _ => {}
        }
        Action::None
    }

    /// 处理一次粘贴。
    pub fn paste(&mut self, text: &str) {
        self.goal_col = None;
        self.follow = true;
        self.editor.insert(text);
    }

    /// 处理一个鼠标事件。`inside` 是这个事件落在输入框的边框之内。
    pub fn mouse(&mut self, event: MouseEvent, inside: bool) -> Action {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) if inside => {
                let pos = self.offset_under(event.column, event.row);
                let double = self
                    .last_click
                    .is_some_and(|(t, p)| p == pos && t.elapsed() < self.double_click);
                if double {
                    let (start, end) = self.editor.word_at(pos);
                    self.editor.select(start, end);
                    self.last_click = None;
                    return Action::None;
                }
                self.editor.select(pos, pos);
                self.dragging = true;
                self.last_click = Some((Instant::now(), pos));
            }
            MouseEventKind::Drag(MouseButton::Left) if self.dragging => {
                // 拖出框的上下边，文字跟着滚。
                if event.row < self.area.y {
                    self.scroll = self.scroll.saturating_sub(1);
                } else if event.row >= self.area.bottom() {
                    self.scroll += 1;
                }
                self.follow = true;
                let pos = self.offset_under(event.column, event.row);
                self.editor.move_to(pos, true);
            }
            // 松开只留着选区，不复制：按 Ctrl+C 才复制（`tui.md`「鼠标」）。
            MouseEventKind::Up(MouseButton::Left) if self.dragging => self.dragging = false,
            MouseEventKind::ScrollUp if inside => {
                self.follow = false;
                self.scroll = self.scroll.saturating_sub(1);
            }
            MouseEventKind::ScrollDown if inside => {
                self.follow = false;
                self.scroll += 1;
            }
            _ => {}
        }
        Action::None
    }

    /// Ctrl+C：有选区复制；没有选区但有字，清空；空着不退出，要外面提示用 Ctrl+D 退出。
    fn ctrl_c(&mut self) -> Action {
        if let Some(text) = self.editor.selected_text() {
            return Action::Copy(text.to_string());
        }
        if self.editor.is_empty() {
            return Action::ExitHint;
        }
        self.editor.take();
        Action::None
    }

    /// 有没有暂存着的字：有就在输入框第一行最右边写一个标记。
    pub fn stashed(&self) -> bool {
        self.stash.is_some()
    }

    /// Ctrl+S 暂存（照旧版）：有字存起来、清空；空着取回来；两边都有互换。
    fn swap_stash(&mut self) {
        let current = (!self.editor.is_empty()).then(|| self.editor.take());
        if let Some(back) = std::mem::replace(&mut self.stash, current) {
            self.editor.insert(&back);
        }
    }

    fn vertical(&mut self, delta: isize, extend: bool) {
        let lines = self.lines(self.area.width);
        let text = self.editor.text();
        let (row, col) = locate(text, &lines, self.editor.cursor());
        let goal = *self.goal_col.get_or_insert(col);
        let target = row.checked_add_signed(delta).filter(|&r| r < lines.len());
        let pos = match target {
            Some(r) => offset_at(text, &lines, r, goal),
            // 第一行再往上、最后一行再往下：翻输入历史；按着 Shift 的照旧到开头、结尾。
            None if !extend => return self.browse(delta),
            None if delta < 0 => 0,
            None => text.len(),
        };
        self.editor.move_to(pos, extend);
    }

    /// 翻输入历史：`delta` 小于 0 往旧的翻。翻过最新一条，回到开始翻之前没发的那句。
    fn browse(&mut self, delta: isize) {
        let newest = self.history.len();
        let at = self.browsing.unwrap_or(newest);
        let Some(next) = at.checked_add_signed(delta).filter(|&n| n <= newest) else {
            return;
        };
        if self.browsing.is_none() {
            self.draft = self.editor.text().to_string();
        }
        self.goal_col = None;
        if next == newest {
            self.browsing = None;
            let draft = std::mem::take(&mut self.draft);
            self.editor.set(&draft);
        } else {
            self.browsing = Some(next);
            self.editor.set(&self.history[next].clone());
        }
    }

    /// 撤销成了：撤掉的那句放回来，整段选中，直接打字就替换掉它（打 `/redo` 不会拼在后面）；
    /// 框里已经有字的不动（`tui.md`「输入框」第 7 条）。
    pub fn put_back(&mut self, said: &str) {
        if !self.editor.is_empty() {
            return;
        }
        self.editor.set(said);
        self.editor.select_all();
        self.put_back = Some(said.to_string());
    }

    /// 恢复了：撤销时放回来的那句还没动过的，收回去，免得回车再发一遍（`tui.md`「正文」第 6 条）。
    pub fn take_back(&mut self) {
        if self
            .put_back
            .take()
            .is_some_and(|said| self.editor.text() == said)
        {
            self.editor.take();
        }
    }

    /// 发过的话和命令，从旧到新（输入历史列表用）。
    pub fn sent(&self) -> &[String] {
        &self.history
    }

    /// 从输入历史列表里挑了一条：放进输入框，光标在末尾。框里原来有字的先存进暂存；暂存里已经有字的，
    /// 原来的字记进输入历史（`↑` 翻得到），不丢（`tui.md`「输入历史列表」第 4 条）。
    pub fn pick(&mut self, text: &str) {
        let current = self.editor.text().to_string();
        if !current.is_empty() && current != text {
            if self.stash.is_none() {
                self.stash = Some(current);
            } else {
                self.remember(&current);
            }
        }
        self.browsing = None;
        self.editor.set(text);
    }

    /// 记下一句发出去的话或命令，翻历史用。和上一条一样的不重复记。
    pub fn remember(&mut self, text: &str) {
        if self.history.last().is_none_or(|last| last != text) {
            self.history.push(text.to_string());
        }
    }

    fn line_edge(&mut self, end: bool, extend: bool) {
        let lines = self.lines(self.area.width);
        let (row, _) = locate(self.editor.text(), &lines, self.editor.cursor());
        let line = lines[row];
        self.editor
            .move_to(if end { line.end } else { line.start }, extend);
    }

    /// 屏幕坐标下的文字下标；在文字区外面的，贴到最近的边上。
    fn offset_under(&self, column: u16, row: u16) -> usize {
        let lines = self.lines(self.area.width);
        let a = self.area;
        let r = usize::from(row.clamp(a.y, a.bottom().saturating_sub(1)) - a.y) + self.scroll;
        if row < a.y {
            return offset_at(self.editor.text(), &lines, self.scroll, 0);
        }
        let c = usize::from(column.saturating_sub(a.x));
        offset_at(self.editor.text(), &lines, r, c)
    }
}

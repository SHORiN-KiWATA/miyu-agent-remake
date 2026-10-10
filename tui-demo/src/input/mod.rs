//! 输入框：文字编辑、折行、滚动、键盘和鼠标。
//!
//! 画在哪、多大由 `ui` 定；这里只记着上一次画的位置，好把鼠标坐标换回文字下标。

mod attach;
mod dropped;
mod editor;
mod mouse;
mod pasted;
mod place;
mod recall;
mod saved;
mod wrap;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Position, Rect};

pub use attach::{AttachKind, AttachRule, Attachment};
use dropped::Dropped;
pub use editor::Editor;
pub use pasted::{Block, Draft, PasteRule, Sent};
pub use saved::{Saved, place};
pub use wrap::{VisualLine, locate, offset_at, pieces, tail_pieces, wrap, wrap_words};

/// 输入框处理完一个事件后，要外面做的事。
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// 什么都不用做。
    None,
    /// 发出去这段话（连同里面的粘贴块，`Draft::expand` 是全文）。
    Submit(Draft),
    /// 把这段字放进剪贴板。
    Copy(String),
    /// 退出程序。
    Quit,
    /// 空着按了 `Ctrl+C`：不退出，提示用 `Ctrl+D` 退出。
    ExitHint,
    /// `Ctrl+C` 清空了输入框：清掉的留着一份，提示按 `↑` 找回。
    Cleared,
    /// 点了输入框里的附件块：用系统的程序打开这个文件（「输入框」第 12 条）。
    Open(String),
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
    /// `Ctrl+S` 暂存的字，连同粘贴块。
    stash: Option<Draft>,
    /// 发过的话和命令，从旧到新，连同粘贴块和发出去的时刻：翻历史、历史列表用。
    history: Vec<Sent>,
    /// 输入历史记在哪个文件（`saved.rs`）。
    saved: Saved,
    /// 正在翻历史，翻到第几条；没在翻是 `None`。
    browsing: Option<usize>,
    /// 开始翻历史之前没发的那句：翻过最新一条回到它。
    draft: Draft,
    /// 画的时候把光标滚进可见范围。滚轮滚过以后关掉，不然滚一下就被拽回光标那里。
    follow: bool,
    /// 撤销时放回来的那句：恢复时它还没动过的话收回去。
    put_back: Option<String>,
    /// 大段粘贴什么时候收成一块、块上写什么（`pasted.rs`）。
    paste_rule: PasteRule,
    /// 在编辑上一句（`/edit`）：改之前的那句，回车时外面拿去比附件换没换（「输入框」第 13 条）。
    editing: Option<Draft>,
    /// `Ctrl+C` 刚清掉的那一句，只留最近一份：空着按 `↑` 先拿回它；发出去一句话、再清一次就换掉。
    cleared: Option<Draft>,
    /// 在附件块上按下去、还没拖：松开时打开这个文件，光标不动（`mouse.rs`）。
    pressed: Option<(std::path::PathBuf, usize)>,
    /// 鼠标悬停在哪个附件块上（字节范围）：画的时候加下划线，指针变手。
    hover: Option<(usize, usize)>,
    /// 附件认哪几种、块上写什么（`attach.rs`）。
    attach_rule: AttachRule,
    /// 这个会话的模型收不了的那几种附件：拖进来、贴进来时收成文件块（「输入框」第 12 条「照模型收」）。
    refused: Vec<String>,
    /// 改成了文件块的那几种，还没提示的。
    demoted: Vec<String>,
    /// 正文里每一种附件已经有几个：输入框里的接着编号（蓝图「输入框」第 12 条）。
    attach_base: HashMap<String, usize>,
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
            saved: Saved::default(),
            browsing: None,
            draft: Draft::default(),
            follow: false,
            put_back: None,
            paste_rule: PasteRule::never(),
            cleared: None,
            editing: None,
            pressed: None,
            hover: None,
            attach_rule: AttachRule::default(),
            refused: Vec::new(),
            demoted: Vec::new(),
            attach_base: HashMap::new(),
        }
    }

    /// 照配置设大段粘贴收成一块的门槛和写法（蓝图「输入框」第 11 条）。
    pub fn set_paste_rule(&mut self, rule: PasteRule) {
        self.paste_rule = rule;
    }

    /// 这个会话的模型收不了的那几种附件（「输入框」第 12 条「照模型收」）。
    pub fn set_refused(&mut self, kinds: Vec<String>) {
        self.refused = kinds;
    }

    /// 拖进来、贴进来时改成了文件块的那几种（提示一句），拿走就清掉。
    pub fn take_demoted(&mut self) -> Vec<String> {
        std::mem::take(&mut self.demoted)
    }

    /// 照配置设附件认哪几种、块上写什么（蓝图「输入框」第 12 条）。
    pub fn set_attach_rule(&mut self, rule: AttachRule) {
        self.attach_rule = rule;
    }

    /// 正文里每一种附件已经有几个：输入框里的附件照先后接着编号（每一帧画之前调，蓝图「输入框」第 12 条）。
    pub fn renumber(&mut self, base: HashMap<String, usize>) {
        self.attach_base = base;
        let (rule, base) = (&self.attach_rule, &self.attach_base);
        self.editor
            .renumber(|kind, n| rule.label(kind, base.get(kind).copied().unwrap_or(0) + n));
    }

    /// 现在输入框里的字，连同粘贴块。
    pub fn draft(&self) -> Draft {
        self.editor.draft()
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
        // 字变了，悬停着的块的位置可能不对了：等鼠标再动一下重新认。
        self.hover = None;
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
                return Action::Submit(self.editor.take_draft());
            }
            // Ctrl+Shift+C 在认得 kitty 键盘协议的终端里报成大写的 C。
            KeyCode::Char('c' | 'C') if ctrl => return self.ctrl_c(),
            KeyCode::Char('d') if ctrl && self.editor.is_empty() => return Action::Quit,
            // 回到输入框开头（2026-09-30 项目主人改：原来是全选）。
            KeyCode::Char('a') if ctrl => self.editor.move_to(0, false),
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
            // 编辑上一句时 `Esc` 是取消：不编辑了，框里清空（「输入框」第 13 条）。
            KeyCode::Esc if self.editing() => self.cancel_edit(),
            KeyCode::Esc => self.editor.clear_selection(),
            _ => {}
        }
        Action::None
    }

    /// `Ctrl+V` 读来的是一批本机现成的文件、目录（文件管理器里复制的、复制的路径）：认得出种类的收成附件，别的收成
    /// 文件块，交回 `true`；不是的什么都不做，交回 `false`（蓝图「输入框」第 12 条）。
    pub fn paste_files(&mut self, text: &str) -> bool {
        let home = std::env::var("HOME").ok();
        let Some(items) = dropped::dropped(text, home.as_deref(), &self.attach_rule) else {
            return false;
        };
        self.goal_col = None;
        self.follow = true;
        for (i, item) in items.into_iter().enumerate() {
            if i > 0 {
                self.editor.insert(" ");
            }
            match item {
                Dropped::File(path, _) | Dropped::Path(path) => self.put_file(path),
            }
        }
        true
    }

    /// 处理一次粘贴（终端送来的：`Ctrl+Shift+V`、拖文件进来）：照原样是字，路径也是字（2026-10-10 项目主人：
    /// 「ctrl+shift+V 粘贴文字，ctrl+V 粘贴占位符」）；大段的收成一块（蓝图「输入框」第 11 条）。
    pub fn paste(&mut self, text: &str) {
        self.goal_col = None;
        self.follow = true;
        let clean = editor::clean(text);
        if self.paste_rule.folds(&clean) {
            let label = self.paste_rule.label(&clean);
            self.editor.insert_block(label, clean);
        } else {
            self.editor.insert(&clean);
        }
    }

    /// 放一个附件（拖进来的、剪贴板里的）：输入框里出一块，编号照先后接着正文里这一种已有的（「输入框」第 12 条）。
    pub fn attach(&mut self, file: std::path::PathBuf, kind: &str) {
        self.goal_col = None;
        self.follow = true;
        // 模型收不了这一种：收成文件块，发出去写路径（「照模型收」）。
        if self.refuses(kind) {
            return self.put_path(file);
        }
        let label = self.attach_rule.label(kind, 0);
        let kind = kind.to_string();
        self.editor
            .insert_attachment(label, Attachment { file, kind });
        let base = std::mem::take(&mut self.attach_base);
        self.renumber(base);
    }

    /// Ctrl+C：有选区复制；没有选区但有字，清空；空着不退出，要外面提示用 Ctrl+D 退出。
    fn ctrl_c(&mut self) -> Action {
        // 复制的是原文：选中的粘贴块换回来（`tui.md`「输入框」第 11 条）。
        if let Some(text) = self.editor.selected_full() {
            return Action::Copy(text);
        }
        if self.editor.is_empty() {
            return Action::ExitHint;
        }
        // 清掉的连同块、附件留最近一份，不进输入历史；空着按 `↑` 先拿回它（「按键」`Ctrl+C`）。正在翻历史的照旧，
        // 翻之前没发的那句还在，翻过最新一条回到它。
        self.cleared = Some(self.editor.take_draft());
        self.editing = None;
        Action::Cleared
    }

    /// 有没有暂存着的字：有就在输入框第一行最右边写一个标记。
    pub fn stashed(&self) -> bool {
        self.stash.is_some()
    }

    /// Ctrl+S 暂存（照旧版）：有字存起来、清空；空着取回来；两边都有互换。
    fn swap_stash(&mut self) {
        let current = (!self.editor.is_empty()).then(|| self.editor.take_draft());
        if let Some(back) = std::mem::replace(&mut self.stash, current) {
            self.editor.set_draft(back);
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
        // 空着、没在翻：`Ctrl+C` 刚清掉的那一句排在输入历史前面，先拿回它（「按键」`Ctrl+C`）。
        if delta < 0
            && self.browsing.is_none()
            && self.editor.is_empty()
            && let Some(draft) = self.cleared.take()
        {
            self.goal_col = None;
            self.editor.set_draft(draft);
            return;
        }
        let newest = self.history.len();
        let at = self.browsing.unwrap_or(newest);
        let Some(next) = at.checked_add_signed(delta).filter(|&n| n <= newest) else {
            return;
        };
        if self.browsing.is_none() {
            self.draft = self.editor.draft();
        }
        self.goal_col = None;
        if next == newest {
            self.browsing = None;
            let draft = std::mem::take(&mut self.draft);
            self.editor.set_draft(draft);
        } else {
            self.browsing = Some(next);
            self.editor.set_draft(self.history[next].draft.clone());
        }
    }

    /// 发过的话和命令，从旧到新（输入历史列表用）。
    pub fn sent(&self) -> &[Sent] {
        &self.history
    }

    /// 从输入历史列表里挑了一条：放进输入框，光标在末尾。框里原来有字的先存进暂存；暂存里已经有字的，
    /// 原来的字记进输入历史（`↑` 翻得到），不丢（`tui.md`「输入历史列表」第 4 条）。
    pub fn pick(&mut self, text: &str) {
        let current = self.editor.draft();
        if !current.text.is_empty() && current.text != text {
            if self.stash.is_none() {
                self.stash = Some(current);
            } else {
                self.remember(current);
            }
        }
        self.browsing = None;
        // 挑中的是记着的那一条：连同粘贴块放回来（同样的字记过几次的，取最近那次）。
        let picked = self
            .history
            .iter()
            .rev()
            .find(|s| s.draft.text == text)
            .map(|s| s.draft.clone());
        self.editor
            .set_draft(picked.unwrap_or_else(|| Draft::plain(text)));
    }

    /// 记下一句发出去的话或命令，连同粘贴块，翻历史用。和上一条一样的不重复记。
    pub fn remember(&mut self, sent: Draft) {
        if self.history.last().is_none_or(|last| last.draft != sent) {
            let sent = Sent {
                draft: sent,
                at: saved::now(),
            };
            // 记进文件：重启以后、别的会话里都翻得到（「输入历史列表」第 8 条）。
            self.saved.append(&sent);
            self.history.push(sent);
        }
    }

    /// 输入历史记在这个文件里：读回最近的 `keep` 条，以后发的接着记（「输入历史列表」第 8 条）。
    pub fn keep_history(&mut self, saved: Saved, keep: usize) {
        let mut earlier = saved.load(keep);
        earlier.append(&mut self.history);
        self.history = earlier;
        self.saved = saved;
    }

    fn line_edge(&mut self, end: bool, extend: bool) {
        let lines = self.lines(self.area.width);
        let (row, _) = locate(self.editor.text(), &lines, self.editor.cursor());
        let line = lines[row];
        self.editor
            .move_to(if end { line.end } else { line.start }, extend);
    }
}

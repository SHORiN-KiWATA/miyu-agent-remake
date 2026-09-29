//! 输入框里的文字：一段字符串、一个光标、一个选区的起点。
//!
//! 光标和选区都是字节下标，永远落在字素簇的边界上：左右移动、删除都按用户看到的
//! 「一个字」走，不会把 emoji 或带组合符号的字切成两半。

use unicode_segmentation::UnicodeSegmentation;

use super::pasted::{Block, Draft};

/// 输入框的文字和光标。换行、滚动这些跟屏幕有关的事不在这里，见 `wrap.rs`。
#[derive(Debug, Default)]
pub struct Editor {
    text: String,
    /// 光标的字节下标。
    cursor: usize,
    /// 选区的另一头。和光标相等时等于没有选区。
    anchor: Option<usize>,
    /// 字里的粘贴块，按位置记着，编辑时跟着挪（`pasted.rs`）：不靠认字，两块写出来一样也各是各的。
    blocks: Vec<Block>,
}

impl Editor {
    /// 全部文字。
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 光标的字节下标。
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// 输入框是不是空的。
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// 选中的字节范围，前小后大；没选中东西时是 `None`。
    pub fn selection(&self) -> Option<(usize, usize)> {
        let anchor = self.anchor.filter(|&a| a != self.cursor)?;
        Some((anchor.min(self.cursor), anchor.max(self.cursor)))
    }

    /// 在光标处插入文字；有选区时先替换掉选区，插完不留选区。
    ///
    /// 粘贴进来的 `\r\n` 统一成 `\n`，制表符换成四个空格（宽度好算），
    /// 其余控制字符丢掉：它们在终端里画出来会把版面搅乱。
    pub fn insert(&mut self, input: &str) {
        self.delete_selection();
        let clean = clean(input);
        self.put(&clean);
    }

    /// 在光标处放一段字（光标不会在块中间）：后面的块跟着挪；放完不留选区——点一下输入框会记下选区的起点，
    /// 不清掉的话光标一挪，新放进来的字就成了选中的样子。
    fn put(&mut self, text: &str) {
        let at = self.cursor;
        for b in self.blocks.iter_mut().filter(|b| b.start >= at) {
            b.start += text.len();
            b.end += text.len();
        }
        self.text.insert_str(at, text);
        self.cursor += text.len();
        self.anchor = None;
    }

    /// 在光标处放一块粘贴：输入框里写 `label`，原文记着，发出去时换回来（蓝图「输入框」第 11 条）。
    pub fn insert_block(&mut self, label: String, text: String) {
        self.delete_selection();
        let start = self.cursor;
        self.put(&label);
        let at = self.blocks.partition_point(|b| b.start < start);
        self.blocks.insert(
            at,
            Block {
                start,
                end: start + label.len(),
                text,
            },
        );
    }

    /// 输入框的字里每一块占的字节范围：画的时候上色用。
    pub fn blocks(&self) -> Vec<(usize, usize)> {
        self.blocks.iter().map(|b| (b.start, b.end)).collect()
    }

    /// 现在的字连同块。
    pub fn draft(&self) -> Draft {
        Draft {
            text: self.text.clone(),
            blocks: self.blocks.clone(),
        }
    }

    /// 拿走字连同块，输入框清空。
    pub fn take_draft(&mut self) -> Draft {
        let blocks = std::mem::take(&mut self.blocks);
        Draft {
            text: self.take(),
            blocks,
        }
    }

    /// 换成一段带着块的字，光标放到末尾：翻历史、取回暂存、放回退回的消息时用。
    pub fn set_draft(&mut self, draft: Draft) {
        self.cursor = draft.text.len();
        self.anchor = None;
        self.text = draft.text;
        self.blocks = draft.blocks;
    }

    /// 选中的字的全文：选区里的块换回原文（复制用）。
    pub fn selected_full(&self) -> Option<String> {
        let (s, e) = self.selection()?;
        Some(self.draft().expand_range(s, e))
    }

    /// 退格：有选区删选区，否则删光标前的一个字；光标前是一块的删整块。
    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        let start = self.prev_boundary(self.cursor);
        self.remove(start, self.cursor);
    }

    /// 删除键：有选区删选区，否则删光标后的一个字；光标后是一块的删整块。
    pub fn delete(&mut self) {
        if self.delete_selection() {
            return;
        }
        let end = self.next_boundary(self.cursor);
        self.remove(self.cursor, end);
    }

    /// 把光标挪到 `pos`。`extend` 为真时保留（或开始）选区，像按着 Shift 移动。
    pub fn move_to(&mut self, pos: usize, extend: bool) {
        if extend {
            self.anchor.get_or_insert(self.cursor);
        } else {
            self.anchor = None;
        }
        self.cursor = self.snap(pos.min(self.text.len()));
    }

    /// 向左一个字。不扩选区时，有选区就收到选区的左头，和常见编辑器一样。
    pub fn left(&mut self, extend: bool) {
        match self.selection() {
            Some((start, _)) if !extend => self.move_to(start, false),
            _ => self.move_to(self.prev_boundary(self.cursor), extend),
        }
    }

    /// 向右一个字，规则同 [`Editor::left`]。
    pub fn right(&mut self, extend: bool) {
        match self.selection() {
            Some((_, end)) if !extend => self.move_to(end, false),
            _ => self.move_to(self.next_boundary(self.cursor), extend),
        }
    }

    /// 从 `start` 选到 `end`，光标停在 `end`。
    pub fn select(&mut self, start: usize, end: usize) {
        self.anchor = Some(self.snap(start.min(self.text.len())));
        self.cursor = self.snap(end.min(self.text.len()));
    }

    /// 全选。
    pub fn select_all(&mut self) {
        self.select(0, self.text.len());
    }

    /// 取消选区，光标不动。
    pub fn clear_selection(&mut self) {
        self.anchor = None;
    }

    /// 拿走全部文字，输入框清空。
    pub fn take(&mut self) -> String {
        self.anchor = None;
        self.cursor = 0;
        self.blocks.clear();
        std::mem::take(&mut self.text)
    }

    /// `pos` 所在的词的范围，给双击选词用：和 `pos` 处的字同一种的一整串（蓝图 `tui.md`「按键」Ctrl+←）。
    pub fn word_at(&self, pos: usize) -> (usize, usize) {
        let Some(kind) = self.text[pos..].chars().next().map(kind) else {
            return (pos, pos);
        };
        let start = self.text[..pos]
            .char_indices()
            .rev()
            .take_while(|(_, c)| self::kind(*c) == kind)
            .last()
            .map_or(pos, |(i, _)| i);
        let end = self.text[pos..]
            .char_indices()
            .find(|(_, c)| self::kind(*c) != kind)
            .map_or(self.text.len(), |(i, _)| pos + i);
        (start, end)
    }

    /// 按词往左跳：先跳过空白，再跳过一个词。词照 Unicode 的分词边界，中文一个字一段。
    pub fn word_left(&mut self, extend: bool) {
        let at = self.word_start(self.cursor);
        let at = self.inside(at).map_or(at, |(start, _)| start);
        self.move_to(at, extend);
    }

    /// 按词往右跳：先跳过空白，再跳过一个词，落在词尾。
    pub fn word_right(&mut self, extend: bool) {
        let rest = &self.text[self.cursor..];
        let first = rest.char_indices().find(|(_, c)| kind(*c) != Kind::Space);
        let end = first.map_or(rest.len(), |(at, c)| {
            let k = kind(c);
            rest[at..]
                .char_indices()
                .find(|(_, c)| kind(*c) != k)
                .map_or(rest.len(), |(i, _)| at + i)
        });
        let at = self.cursor + end;
        let at = self.inside(at).map_or(at, |(_, end)| end);
        self.move_to(at, extend);
    }

    /// 删掉光标前的一个词，连同它后面到光标的空白（Ctrl+W）。有选区时删选区。
    pub fn delete_word(&mut self) {
        if self.delete_selection() {
            return;
        }
        let start = self.word_start(self.cursor);
        self.remove(start, self.cursor);
    }

    /// 把整段字换成 `text`，光标放到末尾：翻历史、放回退回的消息时用。
    pub fn set(&mut self, text: &str) {
        self.take();
        self.insert(text);
    }

    /// `pos` 往左，跳过空白再跳过一个词，落在那个词的开头。
    fn word_start(&self, pos: usize) -> usize {
        let before = &self.text[..pos];
        let mut chars = before
            .char_indices()
            .rev()
            .skip_while(|(_, c)| kind(*c) == Kind::Space);
        let Some((mut start, c)) = chars.next() else {
            return 0;
        };
        let k = kind(c);
        for (i, c) in chars {
            if kind(c) != k {
                break;
            }
            start = i;
        }
        start
    }

    fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.selection() else {
            return false;
        };
        self.remove(start, end);
        self.anchor = None;
        true
    }

    /// 删掉 `[start, end)`，碰到的块整块删（删到一块的一部分也整块删），后面的块跟着挪，光标落在删掉的地方。
    fn remove(&mut self, mut start: usize, mut end: usize) {
        for (s, e) in self.blocks() {
            if s < end && e > start {
                start = start.min(s);
                end = end.max(e);
            }
        }
        self.blocks.retain(|b| b.end <= start || b.start >= end);
        for b in self.blocks.iter_mut().filter(|b| b.start >= end) {
            b.start -= end - start;
            b.end -= end - start;
        }
        self.text.replace_range(start..end, "");
        self.cursor = start;
        // 删了字，选区的起点（点一下记下的）作废：不清的话删空以后成了「选中第 0 到 1 个字节」，
        // 再删这个选区就越界了（2026-09-30 crash.log）。
        self.anchor = None;
    }

    /// `pos` 落在哪一块的中间（不含两头）。
    fn inside(&self, pos: usize) -> Option<(usize, usize)> {
        self.blocks().into_iter().find(|&(s, e)| s < pos && pos < e)
    }

    /// 落在一块中间的挪到离得近的那一头。
    fn snap(&self, pos: usize) -> usize {
        match self.inside(pos) {
            Some((s, e)) if pos - s <= e - pos => s,
            Some((_, e)) => e,
            None => pos,
        }
    }

    fn prev_boundary(&self, pos: usize) -> usize {
        // 光标前是一块：一下跳到块头。
        if let Some((s, _)) = self
            .blocks()
            .into_iter()
            .find(|&(s, e)| s < pos && pos <= e)
        {
            return s;
        }
        self.text[..pos]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(i, _)| i)
    }

    fn next_boundary(&self, pos: usize) -> usize {
        if let Some((_, e)) = self
            .blocks()
            .into_iter()
            .find(|&(s, e)| s <= pos && pos < e)
        {
            return e;
        }
        self.text[pos..]
            .graphemes(true)
            .next()
            .map_or(pos, |g| pos + g.len())
    }
}

/// 字的种类：按词跳、按词删、双击选词照它分段，连在一起的同一种算一个词。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Space,
    Punct,
    Word,
}

/// 空白、标点（中英文的都算），其余（字母、数字、汉字）算词里的字。所以一串中文到空白或标点为止是一个词。
fn kind(c: char) -> Kind {
    if c.is_whitespace() {
        Kind::Space
    } else if c.is_alphanumeric() || c == '_' {
        Kind::Word
    } else {
        Kind::Punct
    }
}

/// 粘贴、打进来的字：`\r\n`、单独的 `\r` 变 `\n`（kitty 这类终端粘贴时把换行送成 `\r`），制表符变四个空格，
/// 别的控制字符丢掉。
pub fn clean(input: &str) -> String {
    input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter_map(|c| match c {
            '\t' => Some("    ".to_string()),
            '\n' => Some("\n".to_string()),
            c if c.is_control() => None,
            c => Some(c.to_string()),
        })
        .collect()
}

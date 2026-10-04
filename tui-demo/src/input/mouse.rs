//! 输入框的鼠标（蓝图 `tui.md`「鼠标」输入框那一行）：点放光标、拖选字、双击选词、滚轮滚。附件块、文件块点一下用系统的程序
//! 打开那个文件，光标不动；从块上按下去拖是拖选；悬停在块上记着，画的时候加下划线、指针变手（「输入框」第 12 条）。

use std::path::PathBuf;
use std::time::Instant;

use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Position;
use unicode_width::UnicodeWidthStr;

use super::{Action, InputBox, offset_at};

impl InputBox {
    /// 处理一个鼠标事件。`inside` 是这个事件落在输入框的边框之内。
    pub fn mouse(&mut self, event: MouseEvent, inside: bool) -> Action {
        match event.kind {
            MouseEventKind::Moved => {
                let under = self.attachment_under(event.column, event.row);
                self.hover = under.filter(|_| inside).map(|(s, e, _)| (s, e));
            }
            MouseEventKind::Down(MouseButton::Left) if inside => {
                let pos = self.offset_under(event.column, event.row);
                // 按在附件块上：先不放光标，松开时打开文件；拖起来才算拖选。
                if let Some((_, _, file)) = self.attachment_under(event.column, event.row) {
                    self.pressed = Some((file, pos));
                    self.dragging = true;
                    self.last_click = None;
                    return Action::None;
                }
                let double = self
                    .last_click
                    .is_some_and(|(t, p)| p == pos && t.elapsed() < self.double_click);
                if double {
                    let (start, end) = self.editor.word_at(pos);
                    self.editor.select(start, end);
                    self.last_click = None;
                    return self
                        .editor
                        .selected_full()
                        .map_or(Action::None, Action::Copy);
                }
                self.editor.select(pos, pos);
                self.dragging = true;
                self.last_click = Some((Instant::now(), pos));
            }
            MouseEventKind::Drag(MouseButton::Left) if self.dragging => {
                if let Some((_, pos)) = self.pressed.take() {
                    self.editor.select(pos, pos);
                }
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
            // 松开复制原文并保留选区；单击附件仍打开文件（`tui.md`「鼠标」）。
            MouseEventKind::Up(MouseButton::Left) if self.dragging => {
                self.dragging = false;
                if let Some((file, _)) = self.pressed.take() {
                    return Action::Open(file.display().to_string());
                }
                return self
                    .editor
                    .selected_full()
                    .map_or(Action::None, Action::Copy);
            }
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

    /// 鼠标悬停在哪个附件块上（字节范围）：画的时候加下划线，指针变手。
    pub fn hovered_attachment(&self) -> Option<(usize, usize)> {
        self.hover
    }

    /// 鼠标离开了输入框：不再悬停在块上。
    pub fn unhover(&mut self) {
        self.hover = None;
    }

    /// 这一格上的附件块：字节范围和文件。照这一行里块露出来的列算，折行折到一半的也算。
    fn attachment_under(&self, column: u16, row: u16) -> Option<(usize, usize, PathBuf)> {
        let a = self.area;
        if !a.contains(Position::new(column, row)) {
            return None;
        }
        let lines = self.lines(a.width);
        let line = lines.get(usize::from(row - a.y) + self.scroll)?;
        let c = usize::from(column - a.x);
        let text = self.editor.text();
        self.editor.openable().find_map(|(s, e, file)| {
            let (from, to) = (s.max(line.start), e.min(line.end));
            if from >= to {
                return None;
            }
            let left = text[line.start..from].width();
            let right = left + text[from..to].width();
            (left..right)
                .contains(&c)
                .then(|| (s, e, file.to_path_buf()))
        })
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

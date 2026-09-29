//! 行首的引子、内容还剩几列、块与块之间的空行（蓝图 `tui.md`「她的回答：Markdown」第 2、5、6、15 条）。

use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use super::inline::{Folded, Piece};
use super::{Container, MdLine, Renderer};
use crate::theme;

impl MdLine {
    /// 空行：块与块之间的那种（`<details>` 里的空行带着竖线，也算）。图的那一行、`<details>` 的标题那一行不算。
    pub(super) fn is_blank(&self) -> bool {
        self.folded.spans.is_empty()
            && self.figure.is_none()
            && self.details.is_none()
            && self
                .lead
                .iter()
                .all(|s| s.content.chars().all(|c| c == '│' || c == ' '))
    }
}

impl Renderer<'_> {
    /// 行首的引子：引用、展开着的 `<details>` 一层一个 `│ `；列表这一项的第一行写记号，别的行写一样宽的空白。
    pub(super) fn lead(&mut self, first: bool) -> Vec<Span<'static>> {
        let mut spans = Vec::new();
        for container in &mut self.containers {
            match container {
                Container::Quote | Container::Details => {
                    spans.push(Span::styled("│ ", theme::dim()));
                }
                Container::Item { marker, used } => {
                    let text = if first && !*used {
                        marker.clone()
                    } else {
                        " ".repeat(marker.width())
                    };
                    spans.push(Span::styled(text, theme::md_list()));
                    if first {
                        *used = true;
                    }
                }
            }
        }
        spans
    }

    /// 内容还剩几列：减去引子。
    pub(super) fn room(&self) -> u16 {
        let lead: usize = self
            .containers
            .iter()
            .map(|c| match c {
                Container::Quote | Container::Details => 2,
                Container::Item { marker, .. } => marker.width(),
            })
            .sum();
        self.width
            .saturating_sub(u16::try_from(lead).unwrap_or(0))
            .max(1)
    }

    /// 分隔线：暗色的 `─`，内容宽度的三分之一，最短 16、最长 40（第 11 条）。
    pub(super) fn rule(&mut self) {
        self.gap();
        let w = usize::from(self.width / 3).clamp(16, 40);
        self.emit(vec![Piece::new("─".repeat(w), theme::dim())], true);
    }

    /// 块与块之间空一行；开头不空，已经空着的不再空，`<details>` 的标题紧接着里面的内容。
    /// 在 `<details>` 里的空行带着竖线，竖线不断。
    pub(super) fn gap(&mut self) {
        let skip = self
            .out
            .last()
            .is_none_or(|last| last.is_blank() || (self.just_titled && last.details.is_some()));
        if skip {
            return;
        }
        let inside = self
            .containers
            .iter()
            .any(|c| matches!(c, Container::Details));
        let lead = if inside { self.lead(false) } else { Vec::new() };
        self.out.push(MdLine {
            lead,
            folded: Folded::default(),
            copy: true,
            figure: None,
            details: None,
        });
    }
}

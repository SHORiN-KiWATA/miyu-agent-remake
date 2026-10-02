//! mermaid 图的账（蓝图 `tui.md`「图片、公式和 mermaid 图」第 4 条）：每份源码问没问过核心、交回了什么 SVG。排正文时问
//! 一声，没问过的记进要发的单子；主循环每一帧以后把单子交给核心（`app/diagrams.rs`）。这次启动里记着回来的 SVG，不存
//! 磁盘：核心那边按源码缓存着。

use std::collections::{HashMap, HashSet};

use crate::core::{Marks, Rendered};
use crate::theme::DiagramColors;

/// 一份源码现在怎样。
pub enum Asked<'a> {
    /// 问了，还没回来。
    Waiting,
    /// 核心拒了、没编进 mermaid：写源码。
    Refused,
    /// 画好了。
    Got(&'a Rendered),
}

/// 账。
#[derive(Debug, Default)]
pub struct Diagrams {
    /// 回来了的；拒了的是 `None`。
    got: HashMap<String, Option<Rendered>>,
    /// 问了还没回来的。
    waiting: HashSet<String>,
    /// 还没发的。
    ask: Vec<String>,
}

impl Diagrams {
    /// 这份源码的图；没问过的记进单子。
    pub fn diagram(&mut self, source: &str) -> Asked<'_> {
        if let Some(got) = self.got.get(source) {
            return got.as_ref().map_or(Asked::Refused, Asked::Got);
        }
        if self.waiting.insert(source.to_string()) {
            self.ask.push(source.to_string());
        }
        Asked::Waiting
    }

    /// 把单子交出去发。
    pub fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut self.ask)
    }

    /// 单子上有没发的。
    pub fn pending(&self) -> bool {
        !self.ask.is_empty()
    }

    /// 核心交回了一张。
    pub fn got(&mut self, source: String, svg: Option<Rendered>) {
        self.waiting.remove(&source);
        self.got.insert(source, svg);
    }

    /// 核心断开了：还没回来的不会回来了，下次排到时再问。
    pub fn lost(&mut self) {
        self.waiting.clear();
        self.ask.clear();
    }
}

/// 把记号色换成主题色：字 `diagram_text`、线 `diagram_line`、连线标签垫的底 `diagram_backdrop`。
pub fn paint(rendered: &Rendered, colors: DiagramColors) -> String {
    let Marks { label, line, text } = &rendered.marks;
    [
        (text, colors.text),
        (line, colors.line),
        (label, colors.backdrop),
    ]
    .into_iter()
    .fold(rendered.svg.clone(), |svg, (mark, (r, g, b))| {
        svg.replace(mark.as_str(), &format!("#{r:02x}{g:02x}{b:02x}"))
    })
}

#[cfg(test)]
mod tests {
    use super::{Asked, Diagrams, paint};
    use crate::core::{Marks, Rendered};
    use crate::theme::DiagramColors;

    fn rendered() -> Rendered {
        Rendered {
            svg: r##"<svg><rect fill="#070809"/><path stroke="#040506"/><text fill="#010203">A</text></svg>"##
                .to_string(),
            marks: Marks {
                label: "#070809".to_string(),
                line: "#040506".to_string(),
                text: "#010203".to_string(),
            },
        }
    }

    #[test]
    fn a_source_is_asked_once_and_lost_asks_are_asked_again() {
        let mut book = Diagrams::default();
        assert!(matches!(book.diagram("graph TD\nA-->B"), Asked::Waiting));
        assert!(matches!(book.diagram("graph TD\nA-->B"), Asked::Waiting));
        assert_eq!(book.take(), ["graph TD\nA-->B"], "同一份只问一次");
        assert!(!book.pending());
        // 核心断开了：没回来的下次排到时再问。
        book.lost();
        assert!(matches!(book.diagram("graph TD\nA-->B"), Asked::Waiting));
        assert_eq!(book.take().len(), 1);
        book.got("graph TD\nA-->B".to_string(), Some(rendered()));
        assert!(matches!(book.diagram("graph TD\nA-->B"), Asked::Got(_)));
        book.got("坏的".to_string(), None);
        assert!(matches!(book.diagram("坏的"), Asked::Refused));
        assert!(book.take().is_empty(), "回来过的不再问");
    }

    #[test]
    fn the_marks_become_the_theme_colors() {
        let colors = DiagramColors {
            text: (0xa9, 0xb1, 0xd6),
            line: (0x73, 0x7a, 0xa2),
            backdrop: (0x1a, 0x1b, 0x26),
        };
        assert_eq!(
            paint(&rendered(), colors),
            r##"<svg><rect fill="#1a1b26"/><path stroke="#737aa2"/><text fill="#a9b1d6">A</text></svg>"##
        );
    }
}

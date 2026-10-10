//! 中间几步的版面（「第一次打开的引导」第 6、10 条）：顶上一行进度，吉祥物在左、内容在右，整组左右居中，最下面一行
//! 按键提示；窗口窄了收掉吉祥物。好了那一屏的吉祥物从这里的位置走回中间。

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::mascot::Look as MascotLook;
use crate::oobe::Step;
use crate::oobe::look::Look;
use crate::theme;

/// 各块在哪。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Places {
    /// 进度那一行。
    pub bar: Rect,
    /// 吉祥物（没跳的时候）；窗口窄的是 `None`。
    pub mascot: Option<Rect>,
    /// 内容。
    pub content: Rect,
    /// 按键提示那一行。
    pub hints: Rect,
}

/// 照窗口排。
pub fn places(area: Rect, look: &Look, mascot: &MascotLook) -> Places {
    let stage = &look.stage;
    let bar = Rect::new(area.x, area.y + 1, area.width, 1).intersection(area);
    let hints =
        Rect::new(area.x, area.bottom().saturating_sub(2), area.width, 1).intersection(area);
    let body_top = area.y + 3;
    let body_h = hints.y.saturating_sub(body_top + 1);
    // 吉祥物、间隔、内容这一整组，两边再各留 `margin` 列：放不下就收掉吉祥物，只剩内容（第 10 条）。
    let needed = mascot.cols + stage.gap + stage.content_width + 2 * stage.margin;
    let with_mascot = area.width >= needed && body_h >= mascot.rows;
    let content_w = stage.content_width.min(area.width.saturating_sub(4));
    let side = if with_mascot {
        mascot.cols + stage.gap
    } else {
        0
    };
    let group = content_w + side;
    let x = area.x + area.width.saturating_sub(group) / 2;
    let height = stage.max_height.min(body_h);
    let top = body_top + body_h.saturating_sub(height) / 2;
    let content = Rect::new(x + side, top, content_w, height).intersection(area);
    let mascot = with_mascot.then(|| {
        let y = top + height.saturating_sub(mascot.rows) / 2;
        Rect::new(x, y, mascot.cols, mascot.rows)
    });
    Places {
        bar,
        mascot,
        content,
        hints,
    }
}

/// 进度：`● 语言 ── ● 图标 ── ◉ 模型 ── ○ 人格 ── ○ 预设`，走过的暗、这一步强调色加粗、没到的最暗。
pub fn bar(frame: &mut Frame, rect: Rect, step: Step, names: &[String]) {
    let Some(now) = step.bar() else {
        return;
    };
    let mut spans = Vec::new();
    for (i, name) in names.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" ── ", theme::faint()));
        }
        let (mark, style) = match i.cmp(&now) {
            std::cmp::Ordering::Less => ("● ", theme::dim()),
            std::cmp::Ordering::Equal => ("◉ ", theme::accent().add_modifier(Modifier::BOLD)),
            std::cmp::Ordering::Greater => ("○ ", theme::faint()),
        };
        spans.push(Span::styled(format!("{mark}{name}"), style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)).centered(), rect);
}

/// 最下面一行暗色的按键提示，居中。
pub fn hints(frame: &mut Frame, rect: Rect, text: &str) {
    frame.render_widget(
        Paragraph::new(Line::styled(text.to_string(), theme::faint())).centered(),
        rect,
    );
}

/// 最下面一行黄字（没连上核心），居中。
pub fn warn(frame: &mut Frame, rect: Rect, text: &str) {
    frame.render_widget(
        Paragraph::new(Line::styled(text.to_string(), theme::warn())).centered(),
        rect,
    );
}

//! 欢迎页、好了那一屏（「第一次打开的引导」第 8、9、11 条）：星点、大一号的吉祥物、一个字一个字打出来的标题、淡出来的
//! 说明、带流光的「回车开始」。

use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::figure::{self, Extra};
use super::stage;
use super::stars;
use crate::app::App;
use crate::oobe::motion::{ease_out, fraction};
use crate::theme;

/// 吉祥物和下面几行字摆在一起：吉祥物的大小、下面几行；交回吉祥物的那一块（放不下的是 `None`）和字从哪一行起。
fn arrange(area: Rect, cols: u16, rows: u16, gap: u16, lines: u16) -> (Option<Rect>, u16) {
    let total = rows + gap + lines;
    if total > area.height || cols > area.width {
        return (None, area.y + area.height.saturating_sub(lines) / 2);
    }
    let y = area.y + (area.height - total) / 2;
    let rect = Rect::new(area.x + (area.width - cols) / 2, y, cols, rows);
    (Some(rect), y + rows + gap)
}

/// 一行居中的字。
fn centered(frame: &mut Frame, area: Rect, y: u16, line: Line<'static>) {
    if y < area.bottom() {
        frame.render_widget(
            Paragraph::new(line).centered(),
            Rect::new(area.x, y, area.width, 1),
        );
    }
}

/// 带流光的一行（照运行状态行）。
fn shimmer(text: &str, t: f64, app: &App) -> Line<'static> {
    let chars: Vec<char> = text.chars().collect();
    let look = &app.config.layout.shimmer;
    Line::from(
        chars
            .iter()
            .enumerate()
            .map(|(i, c)| Span::styled(c.to_string(), theme::shimmer(i, chars.len(), t, look)))
            .collect::<Vec<_>>(),
    )
}

/// 说明淡出来：前一半最暗、后一半暗色。
fn faded(text: &str, sub: f64) -> Option<Line<'static>> {
    (sub > 0.0).then(|| {
        Line::styled(
            text.to_string(),
            if sub < 0.5 {
                theme::faint()
            } else {
                theme::dim()
            },
        )
    })
}

/// 欢迎页。
pub fn welcome(frame: &mut Frame, area: Rect, app: &mut App, now: Instant) {
    let Some(oobe) = app.oobe.as_ref() else {
        return;
    };
    let look = &app.config.oobe;
    let words = app.config.text.oobe.welcome.clone();
    let chars = words.title.chars().count();
    let scene = oobe.intro.scene(now, &look.intro, chars);
    let base = &app.config.mascot;
    let k = (f64::from(area.height) * look.welcome.height / f64::from(base.rows))
        .min(look.welcome.scale)
        .max(1.0);
    let big = base.scaled(k);
    let (rect, text_y) = arrange(area, big.cols, big.rows, look.welcome.gap, 5);
    let center = rect.map_or((0.5, 0.45), |r| stars::center(area, r));
    let list = oobe.stars.at(now, &look.stars, center);
    let t = now.saturating_duration_since(oobe.born).as_secs_f64();
    stars::draw(frame, area, &list, &look.stars.marks);
    if let Some(rect) = rect {
        let extra = Extra {
            spin: scene.spin,
            dark: scene.dark,
            mouth: scene.mouth,
            ear: scene.ear,
            blink: scene.blink,
            ..Extra::default()
        };
        figure::draw(frame, rect, &big, app, (None, 0.0), extra);
    }
    let title: String = words.title.chars().take(scene.typed).collect();
    let bold = Style::new().add_modifier(Modifier::BOLD);
    centered(frame, area, text_y, Line::styled(title, bold));
    if let Some(line) = faded(&words.sub, scene.sub) {
        centered(frame, area, text_y + 2, line);
    }
    if scene.ready {
        centered(frame, area, text_y + 4, shimmer(&words.start, t, app));
    }
}

/// 好了那一屏：吉祥物从左边走回中间、转一圈；回车以后星点散开、它走到首页的位置。
pub fn done(frame: &mut Frame, area: Rect, app: &mut App, now: Instant) {
    let Some(oobe) = app.oobe.as_ref() else {
        return;
    };
    let look = &app.config.oobe;
    let texts = app.config.text.oobe.clone();
    let fin = oobe.finale(now, &texts);
    let base = app.config.mascot.clone();
    let (middle, text_y) = arrange(area, base.cols, base.rows, look.welcome.gap, 5);
    let from = stage::places(area, look, &base).mascot.or(middle);
    let leave = oobe
        .leaving
        .map(|at| ease_out(fraction(at, now, Duration::from_millis(look.done.leave_ms))));
    let home = crate::ui::home::areas(
        area,
        &|_| 1,
        &app.config.layout,
        (base.cols, base.rows),
        0,
        0,
        0,
    )
    .mascot;
    let rect = match (from, middle) {
        (Some(from), Some(middle)) => {
            let walked = lerp(from, middle, fin.walk);
            Some(match leave {
                Some(p) if home.height > 0 => lerp(middle, home, p),
                _ => walked,
            })
        }
        _ => None,
    };
    let center = middle.map_or((0.5, 0.45), |r| stars::center(area, r));
    let list = oobe.stars.at(now, &look.stars, center);
    let t = now.saturating_duration_since(oobe.born).as_secs_f64();
    let summary = oobe.summary();
    stars::draw(frame, area, &list, &look.stars.marks);
    if let Some(rect) = rect {
        let extra = Extra {
            spin: fin.spin,
            mouth: fin.mouth,
            ..Extra::default()
        };
        figure::draw(frame, rect, &base, app, (None, 0.0), extra);
    }
    if leave.is_some_and(|p| p > 0.3) {
        return;
    }
    let words = &texts.done;
    let title: String = words.title.chars().take(fin.typed).collect();
    centered(
        frame,
        area,
        text_y,
        Line::styled(title, Style::new().add_modifier(Modifier::BOLD)),
    );
    let line = words
        .summary
        .replace("{model}", &summary.model)
        .replace("{persona}", &summary.persona)
        .replace("{preset}", &summary.preset);
    if let Some(line) = faded(&line, fin.sub) {
        centered(frame, area, text_y + 2, line);
    }
    if fin.ready {
        centered(frame, area, text_y + 4, shimmer(&words.start, t, app));
    }
}

/// 两块之间走了 `p` 成。
fn lerp(a: Rect, b: Rect, p: f64) -> Rect {
    let go = |x: u16, y: u16| (f64::from(x) + (f64::from(y) - f64::from(x)) * p).round() as u16;
    Rect::new(go(a.x, b.x), go(a.y, b.y), a.width, a.height)
}

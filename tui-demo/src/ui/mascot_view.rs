//! 画吉祥物（蓝图 `tui.md`「空会话的首页」第 5–8 条）：首页中间、宽屏的侧边栏都用它。顺手定它往哪看：
//! 输入框里有字看输入光标，没字看鼠标指针，没人动时一阵一阵地摇头。

use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::{mascot, theme};

/// 在 `rect` 里画吉祥物（`rect` 是它自己那么大的一块）。`far`：它在侧边栏，看的东西都在左边很远，
/// 虚拟距离放远到屏幕最左边刚好转到头（`tui.md`「后台命令、子代理和侧边栏」第 7 条）。
pub fn draw(frame: &mut Frame, rect: Rect, app: &mut App, far: bool) {
    let now = Instant::now();
    let look = &app.config.mascot;
    let areas_mascot = rect;

    let face = (
        f64::from(areas_mascot.x) + f64::from(look.cols) / 2.0,
        f64::from(areas_mascot.y) + look.center_row,
    );
    let target = if app.input.editor.is_empty() {
        app.pointer
    } else {
        app.input.cursor_position()
    };
    // 没人动时一阵一阵地摇头，代替看鼠标，摇得比跟光标慢（第 8 条）。
    let idling = app.idle.pose(now, &look.idle);
    let half_life = if idling.glance.is_some() {
        look.idle.glance_half_life_ms
    } else {
        look.gaze.half_life_ms
    };
    let gaze = if far {
        mascot::reaching(&look.gaze, face.0)
    } else {
        look.gaze.clone()
    };
    let aim = match idling.glance {
        Some(angles) => angles,
        None => target.map_or((0.0, 0.0), |p| {
            let at = (f64::from(p.x) + 0.5, f64::from(p.y) + 0.5);
            mascot::toward(face, at, look.cell_aspect, &gaze)
        }),
    };
    app.gaze.aim(aim);
    app.gaze.step(now, Duration::from_millis(half_life));
    let (yaw, pitch) = app.gaze.angles();
    let pose = mascot::Pose {
        blink: idling.blink,
        ear: idling.ear,
        ..mascot::Pose::facing(yaw, pitch)
    };
    let lines: Vec<Line> = mascot::render(look, &pose)
        .into_iter()
        .map(|row| {
            let spans: Vec<Span> = row
                .into_iter()
                .map(|cell| match cell {
                    Some(c) => Span::styled(c.mark.to_string(), theme::mascot(c.part)),
                    None => Span::raw(" "),
                })
                .collect();
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), areas_mascot);
}

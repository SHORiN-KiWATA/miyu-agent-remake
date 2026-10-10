//! 吉祥物那一块（蓝图 `tui.md`「空会话的首页」第 10 条，2026-10-11 项目主人）：人格有头像的画头像，图照比例放进这一块、
//! 居中，照画图那一套在后台做，还没做好的先空着；没有头像、画不了的照开关画吉祥物。头像不受吉祥物的开关管。

use std::cell::RefCell;
use std::path::Path;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui_image::sliced::{SignedPosition, SlicedImage};

use crate::app::{App, Avatar, Place};
use crate::figures::{Figures, Look};
use crate::markdown::FigureKind;

/// 画头像画成了没有。
enum Shown {
    /// 画了。
    Yes,
    /// 还在做：这一块先空着。
    Waiting,
    /// 读不出来、终端显示不了：退回吉祥物。
    Unable,
}

/// 这一处有没有那一块：吉祥物的开关开着，或者有头像。
pub fn slot_shown(app: &App, place: Place) -> bool {
    mascot_on(app, place) || app.avatar(place) != Avatar::None
}

/// 吉祥物在这一处画不画（首页、侧边栏各一个开关）。
fn mascot_on(app: &App, place: Place) -> bool {
    match place {
        Place::Home => app.config.layout.mascot_home,
        Place::Sidebar => app.config.layout.mascot_sidebar,
    }
}

/// 在 `rect`（吉祥物那么大的一块）里画：头像，或者吉祥物。`far` 照吉祥物的（在侧边栏）。
pub fn draw(frame: &mut Frame, rect: Rect, app: &mut App, place: Place, far: bool) {
    let shown = match app.avatar(place) {
        Avatar::File(path) => picture(frame, &app.figures, rect, &path),
        Avatar::Waiting => Shown::Waiting,
        Avatar::None => Shown::Unable,
    };
    if matches!(shown, Shown::Unable) && mascot_on(app, place) {
        super::mascot_view::draw(frame, rect, app, far);
    }
}

/// 把图照比例放进 `rect`、居中画。
fn picture(frame: &mut Frame, figures: &RefCell<Figures>, rect: Rect, path: &Path) -> Shown {
    let mut figures = figures.borrow_mut();
    let source = path.display().to_string();
    let look = figures.look(
        FigureKind::Preview,
        &source,
        (None, None),
        rect.width,
        rect.height,
    );
    let (key, rows) = match look {
        Look::Ready { key, rows } => (key, rows.min(rect.height)),
        Look::Pending => return Shown::Waiting,
        Look::Unsupported | Look::Failed => return Shown::Unable,
    };
    let cols = figures.cols(key).unwrap_or(rect.width).min(rect.width);
    let Some(drawn) = figures.shown(key) else {
        return Shown::Waiting;
    };
    let own = Rect::new(
        rect.x + (rect.width - cols) / 2,
        rect.y + (rect.height - rows) / 2,
        cols,
        rows,
    );
    // 整张露在窗口里才画（侧边栏、首页都不会把它切掉一截）。
    if frame.area().contains(own.as_position()) && own.bottom() <= frame.area().bottom() {
        SlicedImage::new(&drawn.protocol, SignedPosition { x: 0, y: 0 })
            .render(own, frame.buffer_mut());
    }
    Shown::Yes
}

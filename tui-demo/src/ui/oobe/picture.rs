//! 引导里画一张小图（建人格的头像，「第一次打开的引导」第 21 条）：照画图那一套在后台做（`figures`），好了只画进它占的那几行，
//! 被滚出这一块的切掉（同正文里的图，「图片、公式和 mermaid 图」第 6a 条）。终端显示不了图的、还没做好的、读不出来的不画。

use std::cell::RefCell;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use ratatui_image::sliced::{SignedPosition, SlicedImage};

use super::content::Picture;
use crate::figures::{Figures, Look};
use crate::markdown::FigureKind;

/// 画进 `area`（这一步右边那一块，已经往上滚了 `scroll` 行）。
pub fn draw(
    frame: &mut Frame,
    figures: &RefCell<Figures>,
    area: Rect,
    scroll: u16,
    picture: &Picture,
) {
    let mut figures = figures.borrow_mut();
    if !figures.shows() {
        return;
    }
    let look = figures.look(
        FigureKind::Preview,
        &picture.path,
        (None, None),
        picture.cols,
        picture.rows,
    );
    let Look::Ready { key, .. } = look else {
        return;
    };
    let Some(drawn) = figures.shown(key) else {
        return;
    };
    // 它占的那几行露在这一块里的一截（相对 `area` 的行）。
    let top = i32::from(picture.line) - i32::from(scroll);
    let first = top.max(0);
    let last = (top + i32::from(picture.rows)).min(i32::from(area.height));
    if last <= first || picture.x >= area.width {
        return;
    }
    let own = Rect::new(
        area.x + picture.x,
        area.y + u16::try_from(first).unwrap_or(0),
        area.width - picture.x,
        u16::try_from(last - first).unwrap_or(0),
    );
    let position = SignedPosition {
        x: 0,
        y: i16::try_from(top - first).unwrap_or(0),
    };
    SlicedImage::new(&drawn.protocol, position).render(own, frame.buffer_mut());
}

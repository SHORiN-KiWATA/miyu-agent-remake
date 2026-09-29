//! 出图的后台线程（蓝图 `tui.md`「图片、公式和 mermaid 图」第 6 条）：读文件、出 SVG、栅格化、
//! 排公式、按终端的协议编码，一张张做，做好了经 `notify` 交回主循环。

use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use image::DynamicImage;
use ratatui::layout::Size;
use ratatui_image::Resize;
use ratatui_image::sliced::SlicedProtocol;

use super::terminal::Graphics;
use super::{Drawn, file, math, mermaid, svg};
use crate::config::FigureLook;
use crate::markdown::FigureKind;
use crate::theme::DiagramColors;

/// 后台线程的名字：它里面的 panic 不收拾终端（见 [`quiet_panics`]）。
const NAME: &str = "miyu-figures";

/// 一张要做的图。
pub struct Job {
    /// 做好了按它认回来。
    pub key: u64,
    /// 哪一种。
    pub kind: FigureKind,
    /// 源码。
    pub source: String,
    /// `<img>` 写的宽高（像素）。
    pub size: crate::markdown::Size,
    /// 最多几列宽。
    pub cols: u16,
    /// 公式的字色。
    pub math: (u8, u8, u8),
    /// mermaid 图的颜色。
    pub diagram: DiagramColors,
}

/// 做完的一张：画好了，或者出错的原因。
pub struct Done {
    /// 哪一张。
    pub key: u64,
    /// 结果。
    pub result: Result<Drawn, String>,
}

/// 起后台线程，交回送活的口子。`notify` 交回假时（主循环没了）线程收工。
/// `zoom_dir` 是点开看的 mermaid 大图放在哪；没有的不出那一行。
pub fn spawn(
    graphics: Graphics,
    look: FigureLook,
    zoom_dir: Option<PathBuf>,
    notify: impl Fn(Done) -> bool + Send + 'static,
) -> mpsc::Sender<Job> {
    quiet_panics();
    let (sender, jobs) = mpsc::channel::<Job>();
    let started = thread::Builder::new()
        .name(NAME.to_string())
        .spawn(move || {
            for job in jobs {
                let key = job.key;
                // 渲染库碰到怪输入可能 panic：接住，当出错，界面写源码。
                let result = panic::catch_unwind(AssertUnwindSafe(|| {
                    draw(&graphics, &look, zoom_dir.as_deref(), &job)
                }))
                .unwrap_or_else(|_| Err("出图时崩了".to_string()));
                if !notify(Done { key, result }) {
                    return;
                }
            }
        });
    if started.is_err() {
        // 线程起不来：送出去的活没人做，图一直是占位；界面照常用。
    }
    sender
}

fn draw(
    graphics: &Graphics,
    look: &FigureLook,
    zoom_dir: Option<&Path>,
    job: &Job,
) -> Result<Drawn, String> {
    let cell = graphics.cell();
    let mut zoom = None;
    let (image, fit) = match job.kind {
        FigureKind::Image => file::draw(&job.source, job.size, cell, job.cols, look.max_rows)?,
        FigureKind::Svg => svg::draw(&job.source, &look.fonts, cell, job.cols, look.max_rows)?,
        FigureKind::Mermaid => {
            let style = mermaid::Look {
                colors: job.diagram,
                fonts: &look.fonts,
            };
            let drawn = mermaid::draw(&job.source, &style, cell, job.cols, look.max_rows)?;
            // 大图写不进缓存目录也不要紧：图照画，只是没有「点开看大图」那一行。
            zoom = zoom_dir
                .and_then(|dir| mermaid::zoom(&job.source, &style, dir, look.zoom_keep).ok());
            drawn
        }
        FigureKind::Math => math::draw(
            &job.source,
            job.math,
            look.math_scale,
            cell,
            job.cols,
            look.max_rows,
        )?,
    };
    let size = Size::new(fit.cols, fit.rows);
    let image = DynamicImage::ImageRgba8(image);
    let protocol =
        SlicedProtocol::new_with_resize(&graphics.picker, image, size, Resize::Fit(None))
            .map_err(|e| e.to_string())?;
    Ok(Drawn {
        protocol,
        rows: fit.rows,
        zoom,
    })
}

/// 后台线程里的 panic 不走 ratatui 装的那一套（那一套会把终端退出全屏、打出报错）：
/// 它由 `catch_unwind` 接住，当这张图出错。别的线程照旧。
fn quiet_panics() {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if thread::current().name() != Some(NAME) {
            previous(info);
        }
    }));
}

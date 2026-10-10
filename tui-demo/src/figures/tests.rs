//! 记着做好的图（蓝图 `tui.md`「图片、公式和 mermaid 图」第 6 条）：同一张不重做，宽度变了重做；编好的图记满了
//! 扔最久没露出来的，占几行一直记着；终端显示不了图的一律写源码。

use std::sync::mpsc;

use image::{DynamicImage, RgbaImage};
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::sliced::SlicedProtocol;

use super::worker::{Done, Job, Outcome};
use super::{Drawn, Figures, Look};
use crate::markdown::FigureKind;

fn drawn(rows: u16) -> Drawn {
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    let image = DynamicImage::ImageRgba8(RgbaImage::new(40, u32::from(rows) * 20));
    Drawn {
        protocol: SlicedProtocol::new(&picker, image, None).unwrap(),
        rows,
        zoom: None,
    }
}

fn figures(keep: usize) -> (Figures, mpsc::Receiver<Job>) {
    let (sender, jobs) = mpsc::channel();
    (Figures::with_jobs(Some(sender), keep), jobs)
}

#[test]
fn the_same_figure_is_asked_for_once_and_then_drawn() {
    let (mut figures, jobs) = figures(8);
    assert_eq!(
        figures.look(FigureKind::Math, "x^2", (None, None), 40, 20),
        Look::Pending
    );
    assert_eq!(
        figures.look(FigureKind::Math, "x^2", (None, None), 40, 20),
        Look::Pending
    );
    let job = jobs.try_recv().unwrap();
    assert!(jobs.try_recv().is_err(), "同一张只做一次");
    figures.done(Done {
        key: job.key,
        result: Outcome::Drawn(drawn(3)),
    });
    assert_eq!(
        figures.look(FigureKind::Math, "x^2", (None, None), 40, 20),
        Look::Ready {
            key: job.key,
            rows: 3
        }
    );
    assert!(figures.get(job.key).is_some());
    // 宽度变了是另一张，重做；做好以前先照旧画原来那张（2026-09-30）。
    assert_eq!(
        figures.look(FigureKind::Math, "x^2", (None, None), 30, 20),
        Look::Ready {
            key: job.key,
            rows: 3
        }
    );
    assert!(jobs.try_recv().is_ok());
}

#[test]
fn failures_and_terminals_without_images_fall_back_to_source() {
    let (mut figures, jobs) = figures(8);
    figures.look(FigureKind::Mermaid, "坏的", (None, None), 40, 20);
    let job = jobs.try_recv().unwrap();
    figures.done(Done {
        key: job.key,
        result: Outcome::Failed,
    });
    assert_eq!(
        figures.look(FigureKind::Mermaid, "坏的", (None, None), 40, 20),
        Look::Failed
    );
    let mut plain = Figures::with_jobs(None, 8);
    assert_eq!(
        plain.look(FigureKind::Image, "a.png", (None, None), 40, 20),
        Look::Unsupported
    );
}

#[test]
fn more_figures_than_keep_are_made_once_and_the_layout_settles() {
    // 2026-09-29 项目主人报：一条回复里三十多张壁纸，一直「正在画图」、CPU 占满。原来记满了扔最早的、扔一张就
    // 整份重排，重排时刚扔的那张又去重做、挤掉下一张，转个不停。
    let (mut figures, jobs) = figures(2);
    let sources = ["a", "b", "c"];
    let mut made = 0;
    let mut settled = false;
    // 正文每重排一次，每张图都来问一声；后台做好就交回来。图的状态不再变，正文就不再重排。
    for _ in 0..10 {
        let revision = figures.revision();
        for source in sources {
            figures.look(FigureKind::Math, source, (None, None), 40, 20);
        }
        for job in jobs.try_iter() {
            made += 1;
            figures.done(Done {
                key: job.key,
                result: Outcome::Drawn(drawn(1)),
            });
        }
        if figures.revision() == revision {
            settled = true;
            break;
        }
    }
    assert!(settled, "排版停得下来");
    assert_eq!(made, 3, "每张只做一次");
    for source in sources {
        assert!(matches!(
            figures.look(FigureKind::Math, source, (None, None), 40, 20),
            Look::Ready { rows: 1, .. }
        ));
    }
}

#[test]
fn only_pictures_off_screen_are_dropped_and_they_come_back_once_when_seen() {
    let (mut figures, jobs) = figures(1);
    let mut keys = Vec::new();
    for source in ["a", "b"] {
        figures.look(FigureKind::Math, source, (None, None), 40, 20);
        let job = jobs.try_recv().unwrap();
        keys.push(job.key);
        figures.done(Done {
            key: job.key,
            result: Outcome::Drawn(drawn(2)),
        });
    }
    let (a, b) = (keys[0], keys[1]);
    let revision = figures.revision();
    // 这一帧露着 a：记满了，下一帧开始时扔 b 的编码；两张占几行都还记着，排版不变。
    figures.next_frame();
    assert!(figures.shown(a).is_some());
    figures.next_frame();
    assert!(figures.get(a).is_some(), "露着的不扔");
    assert!(figures.get(b).is_none(), "没露出来的扔编码");
    assert!(matches!(
        figures.look(FigureKind::Math, "b", (None, None), 40, 20),
        Look::Ready { rows: 2, .. }
    ));
    assert!(jobs.try_recv().is_err(), "排版问一声不重做");
    // b 露出来了：重做一次，同一帧再问不重复交。
    assert!(figures.shown(b).is_none());
    assert!(figures.shown(b).is_none());
    let redo = jobs.try_recv().unwrap();
    assert_eq!(redo.key, b);
    assert!(jobs.try_recv().is_err(), "同一张在做的不重复交给后台");
    figures.done(Done {
        key: b,
        result: Outcome::Drawn(drawn(2)),
    });
    assert!(figures.shown(b).is_some());
    assert_eq!(figures.revision(), revision, "扔编码、重做都不重排");
}

#[test]
fn after_forgetting_every_figure_is_made_again() {
    let (mut figures, jobs) = figures(8);
    figures.look(FigureKind::Math, "x", (None, None), 40, 20);
    let job = jobs.try_recv().unwrap();
    figures.done(Done {
        key: job.key,
        result: Outcome::Drawn(drawn(1)),
    });
    // 挂起回来：终端可能丢了传过的图，重做一遍、重新传。
    figures.forget();
    assert!(figures.get(job.key).is_none());
    assert_eq!(
        figures.look(FigureKind::Math, "x", (None, None), 40, 20),
        Look::Pending
    );
    assert!(jobs.try_recv().is_ok());
}

#[test]
fn a_different_row_limit_is_another_figure() {
    // 2026-09-30 项目主人：图最多占窗口高度的几分之几。窗口变高变矮，最多几行变了，重做。
    let (mut figures, jobs) = figures(8);
    figures.look(FigureKind::Image, "a.png", (None, None), 40, 20);
    assert_eq!(jobs.try_recv().unwrap().rows, 20, "照给的最多几行做");
    assert_eq!(
        figures.look(FigureKind::Image, "a.png", (None, None), 40, 12),
        Look::Pending
    );
    assert_eq!(jobs.try_recv().unwrap().rows, 12);
}

#[test]
fn a_new_size_keeps_showing_the_old_picture_until_the_new_one_is_ready() {
    // 2026-09-30 项目主人：最大化、调小窗口时图都重新加载一次（闪「正在画图」）。
    let (mut figures, jobs) = figures(8);
    figures.look(FigureKind::Math, "x", (None, None), 40, 20);
    let first = jobs.try_recv().unwrap();
    figures.done(Done {
        key: first.key,
        result: Outcome::Drawn(drawn(2)),
    });
    assert_eq!(
        figures.look(FigureKind::Math, "x", (None, None), 30, 20),
        Look::Ready {
            key: first.key,
            rows: 2
        },
        "窗口变窄：先照旧画原来那张"
    );
    let second = jobs.try_recv().unwrap();
    assert_ne!(second.key, first.key, "新尺寸照样交给后台做");
    figures.done(Done {
        key: second.key,
        result: Outcome::Drawn(drawn(3)),
    });
    assert_eq!(
        figures.look(FigureKind::Math, "x", (None, None), 30, 20),
        Look::Ready {
            key: second.key,
            rows: 3
        },
        "新的做好了换上"
    );
}

#[test]
fn while_resizing_only_the_latest_size_of_each_picture_is_made() {
    let (mut figures, jobs) = figures(8);
    for (source, cols) in [("x", 40), ("y", 40), ("x", 30), ("x", 20)] {
        figures.look(FigureKind::Math, source, (None, None), cols, 20);
    }
    let queued: Vec<Job> = jobs.try_iter().collect();
    let keys: Vec<u64> = queued.iter().map(|j| j.key).collect();
    let (kept, skipped) = super::worker::latest(queued);
    let kept: Vec<u64> = kept.iter().map(|j| j.key).collect();
    assert_eq!(kept, [keys[1], keys[3]], "每张图只做最新的那个尺寸");
    assert_eq!(skipped, [keys[0], keys[2]]);
    // 跳过的交回来：那一张不再算在做，要的时候重新交给后台。
    figures.done(Done {
        key: keys[0],
        result: Outcome::Skipped,
    });
    assert_eq!(
        figures.look(FigureKind::Math, "x", (None, None), 40, 20),
        Look::Pending
    );
    assert_eq!(jobs.try_recv().unwrap().key, keys[0]);
}

#[test]
fn a_kitty_picture_tells_the_terminal_how_many_cells_it_spans() {
    // 2026-10-09 项目主人报：`read` 读的图比留的行大，盖住了上面的正文。虚拟放置不写占几列几行，终端照像素和它自己的
    // 格子推，格子大小和启动时问出来的对不上就推错（蓝图「图片、公式和 mermaid 图」第 2 条）。
    use ratatui::buffer::Buffer;
    use ratatui::layout::{Rect, Size};
    use ratatui::widgets::Widget;
    use ratatui_image::Resize;
    use ratatui_image::sliced::{SignedPosition, SlicedImage};

    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    let image = DynamicImage::ImageRgba8(RgbaImage::new(40, 60));
    let protocol =
        SlicedProtocol::new_with_resize(&picker, image, Size::new(4, 3), Resize::Fit(None))
            .unwrap();
    let area = Rect::new(0, 0, 10, 5);
    let mut buf = Buffer::empty(area);
    SlicedImage::new(&protocol, SignedPosition { x: 0, y: 0 }).render(area, &mut buf);
    let sent: String = buf.content.iter().map(|c| c.symbol()).collect();
    assert!(sent.contains("U=1"), "是 kitty 的虚拟放置");
    assert!(sent.contains("c=4,r=3"), "带占几列几行");
}

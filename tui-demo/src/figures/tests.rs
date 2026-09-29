//! 记着做好的图（蓝图 `tui.md`「图片、公式和 mermaid 图」第 6 条）：同一张不重做，宽度变了重做，
//! 记满了扔最早的；终端显示不了图的一律写源码。

use std::sync::mpsc;

use image::{DynamicImage, RgbaImage};
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::sliced::SlicedProtocol;

use super::worker::{Done, Job};
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
    assert_eq!(figures.look(FigureKind::Math, "x^2", 40), Look::Pending);
    assert_eq!(figures.look(FigureKind::Math, "x^2", 40), Look::Pending);
    let job = jobs.try_recv().unwrap();
    assert!(jobs.try_recv().is_err(), "同一张只做一次");
    figures.done(Done {
        key: job.key,
        result: Ok(drawn(3)),
    });
    assert_eq!(
        figures.look(FigureKind::Math, "x^2", 40),
        Look::Ready {
            key: job.key,
            rows: 3
        }
    );
    assert!(figures.get(job.key).is_some());
    // 宽度变了是另一张，重做。
    assert_eq!(figures.look(FigureKind::Math, "x^2", 30), Look::Pending);
    assert!(jobs.try_recv().is_ok());
}

#[test]
fn failures_and_terminals_without_images_fall_back_to_source() {
    let (mut figures, jobs) = figures(8);
    figures.look(FigureKind::Mermaid, "坏的", 40);
    let job = jobs.try_recv().unwrap();
    figures.done(Done {
        key: job.key,
        result: Err("读不懂".to_string()),
    });
    assert_eq!(figures.look(FigureKind::Mermaid, "坏的", 40), Look::Failed);
    let mut plain = Figures::with_jobs(None, 8);
    assert_eq!(
        plain.look(FigureKind::Image, "a.png", 40),
        Look::Unsupported
    );
}

#[test]
fn a_full_store_drops_the_oldest() {
    let (mut figures, jobs) = figures(2);
    for source in ["a", "b", "c"] {
        figures.look(FigureKind::Math, source, 40);
    }
    let first = jobs.try_recv().unwrap();
    // 最早的那张被扔了：做完回来也不记，再问是重做。
    figures.done(Done {
        key: first.key,
        result: Ok(drawn(1)),
    });
    assert!(figures.get(first.key).is_none());
    jobs.try_iter().for_each(drop);
    assert_eq!(figures.look(FigureKind::Math, "a", 40), Look::Pending);
    assert!(jobs.try_recv().is_ok());
}

#[test]
fn after_forgetting_every_figure_is_made_again() {
    let (mut figures, jobs) = figures(8);
    figures.look(FigureKind::Math, "x", 40);
    let job = jobs.try_recv().unwrap();
    figures.done(Done {
        key: job.key,
        result: Ok(drawn(1)),
    });
    // 挂起回来：终端可能丢了传过的图，重做一遍、重新传。
    figures.forget();
    assert!(figures.get(job.key).is_none());
    assert_eq!(figures.look(FigureKind::Math, "x", 40), Look::Pending);
    assert!(jobs.try_recv().is_ok());
}

//! 看不了图的会话要的几样（施工 8-17，`random_logs.rs` 用；从 `random_logs.rs` 分出来，那边放不下了）。

use miyu_kernel::block::{Block, Image};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::{ContentHash, MediaType};
use miyu_kernel::session::Limits;
use miyu_kernel::testkit::model;

/// 第 `k` 张图，不带名字。
pub fn picture(k: u64) -> Block {
    Block::Image(Image {
        blob: ContentHash::of(format!("picture {k}").as_bytes()),
        name: None,
        media_type: MediaType::parse("image/png").expect("媒体类型合写法"),
        width: 640,
        height: 480,
        path: None,
    })
}

/// 看不了图的限额：窗口照给的，别的没有。
pub fn blind(window: Option<u64>) -> Limits {
    Limits {
        model: model(),
        window,
        max_output: None,
        images: None,
        blind: true,
    }
}

/// 日志里有几条转述。
pub fn described(log: &[Event]) -> usize {
    log.iter()
        .filter(|event| matches!(event.body, Body::ImageDescribed(_)))
        .count()
}

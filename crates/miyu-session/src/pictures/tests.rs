//! 工具交回的图片（施工 4-13）：存成了 blob，块里是它们的哈希、媒体类型、宽高，照先后；存不下来的交回空的。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use miyu_kernel::id::{ContentHash, MediaType};

use super::*;

/// 一个用完就删的临时目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("miyu-pictures-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 一张图：字节、媒体类型、宽高。
fn picture(bytes: &[u8], media_type: &str, width: u32, height: u32) -> Picture {
    Picture {
        bytes: bytes.to_vec(),
        media_type: MediaType::parse(media_type).unwrap(),
        width,
        height,
    }
}

#[test]
fn pictures_become_image_blocks_in_order() {
    let scratch = Scratch::new();
    let blobs = Blobs::new(scratch.0.join("blobs"));
    let blocks = store(
        &blobs,
        vec![
            picture(b"first", "image/png", 3, 2),
            picture(b"second", "image/webp", 1, 1),
        ],
    )
    .expect("存得下");
    let expected = |bytes: &[u8], media_type: &str, width, height| {
        Block::Image(Image {
            blob: ContentHash::of(bytes),
            name: None,
            media_type: MediaType::parse(media_type).unwrap(),
            width,
            height,
        })
    };
    assert_eq!(
        blocks,
        [
            expected(b"first", "image/png", 3, 2),
            expected(b"second", "image/webp", 1, 1),
        ]
    );
    assert_eq!(blobs.get(&ContentHash::of(b"first")).unwrap(), b"first");
    // 没有图的：什么都不加。
    assert_eq!(store(&blobs, Vec::new()), Some(Vec::new()));
}

#[test]
fn a_picture_that_cannot_be_stored_gives_nothing() {
    // blob 的目录是个文件：存不下来。
    let scratch = Scratch::new();
    let blocked = scratch.0.join("blobs");
    std::fs::write(&blocked, b"not a directory").unwrap();
    let blobs = Blobs::new(blocked);
    assert_eq!(store(&blobs, vec![picture(b"x", "image/png", 1, 1)]), None);
}

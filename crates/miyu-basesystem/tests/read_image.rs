//! `read` 读图片（施工 4-13）：四种格式交回图片、不另写字，扩展名不算数；太大的拦下，照样报读过；量不出宽高的当
//! 二进制；`offset`、`limit` 不管。

use miyu_kernel::id::{ContentHash, MediaType};
use miyu_tool::{Done, Effect};

use crate::support::{Site, tool};

/// 一个 PNG 的开头：签名、IHDR 里的宽高。后面的字节量宽高用不上。
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

/// 一个 GIF 的开头：签名、逻辑屏的宽高（小端）。
fn gif(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = b"GIF89a".to_vec();
    bytes.extend(width.to_le_bytes());
    bytes.extend(height.to_le_bytes());
    bytes.extend([0, 0, 0, 0x3B]);
    bytes
}

/// 一个 JPEG：SOI，一段 SOF0 里的高、宽（大端），EOI。
fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08];
    bytes.extend(height.to_be_bytes());
    bytes.extend(width.to_be_bytes());
    bytes.extend([3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1, 0xFF, 0xD9]);
    bytes
}

/// 一个 WebP（VP8X）：画布的宽减一、高减一，各三个字节小端。
fn webp(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"RIFF\x00\x00\x00\x00WEBPVP8X\x0a\x00\x00\x00\x00\x00\x00\x00".to_vec();
    bytes.extend(&(width - 1).to_le_bytes()[..3]);
    bytes.extend(&(height - 1).to_le_bytes()[..3]);
    bytes
}

/// 读 `name`，交回 `Done`。
async fn read(site: &Site, name: &str) -> Done {
    site.done("read", serde_json::json!({ "file_path": name }))
        .await
}

/// 读过一个文件：整份的哈希，没有行的范围。
fn read_effect(site: &Site, name: &str, bytes: &[u8]) -> Effect {
    Effect::Read {
        path: site.real(&format!("work/{name}")),
        lines: None,
        hash: ContentHash::of(bytes),
    }
}

#[tokio::test]
async fn each_kind_comes_back_as_a_picture_whatever_its_name() {
    let site = Site::new();
    let cases = [
        ("a.png", png(3, 2), "image/png", 3, 2),
        ("b.gif", gif(5, 4).to_vec(), "image/gif", 5, 4),
        ("c.jpg", jpeg(32, 16), "image/jpeg", 32, 16),
        ("d.webp", webp(7, 9), "image/webp", 7, 9),
        // 看开头的字节认，不看扩展名。
        ("photo.dat", png(1, 1), "image/png", 1, 1),
    ];
    for (name, bytes, media_type, width, height) in cases {
        site.file(&format!("work/{name}"), &bytes);
        let done = read(&site, name).await;
        assert!(!done.error, "{name}");
        assert!(done.blocks.is_empty(), "只交图，不另写字：{name}");
        assert_eq!(done.images.len(), 1, "{name}");
        let picture = &done.images[0];
        assert_eq!(picture.bytes, bytes, "{name}");
        assert_eq!(
            picture.media_type,
            MediaType::parse(media_type).unwrap(),
            "{name}"
        );
        assert_eq!((picture.width, picture.height), (width, height), "{name}");
        let said = done.human.expect("有说法");
        assert_eq!(said.key, "software/basesystem/read/image");
        assert_eq!(said.fields["width"], width.to_string());
        assert_eq!(said.fields["height"], height.to_string());
        assert_eq!(done.effects, [read_effect(&site, name, &bytes)], "{name}");
    }
    // offset、limit 不管。
    let done = site
        .done(
            "read",
            serde_json::json!({"file_path": "a.png", "offset": 5, "limit": 1}),
        )
        .await;
    assert_eq!(done.images.len(), 1);
}

#[tokio::test]
async fn too_big_files_are_refused_but_read_through_for_the_hash() {
    let site = Site::new();
    // 6 MiB：多出上限一截，读剩下那截算哈希的路子才走得到（变异测试逮到过只算了前面的）。
    let mut big = png(10, 10);
    big.resize(6 * 1024 * 1024, 7);
    site.file("work/big.png", &big);
    let done = read(&site, "big.png").await;
    assert!(done.error);
    assert!(done.images.is_empty());
    assert_eq!(
        site.call("read", serde_json::json!({"file_path": "big.png"}))
            .await
            .1,
        "\"big.png\" is 6.0 MiB, too large to view. Images must be at most 5 MiB. Make a smaller copy with a command and read that.\n"
    );
    let said = done.human.expect("有说法");
    assert_eq!(said.key, "software/basesystem/read/image-too-big");
    assert_eq!(said.fields["size"], "6.0 MiB");
    assert_eq!(done.effects, [read_effect(&site, "big.png", &big)]);
    // 多一个字节就拦；正好 5 MiB 的照读。
    let mut over = png(10, 10);
    over.resize(5 * 1024 * 1024 + 1, 0);
    site.file("work/over.png", &over);
    assert!(read(&site, "over.png").await.error);
    let mut edge = png(10, 10);
    edge.resize(5 * 1024 * 1024, 0);
    site.file("work/edge.png", &edge);
    assert_eq!(read(&site, "edge.png").await.images.len(), 1);
}

#[tokio::test]
async fn too_wide_images_are_refused() {
    let site = Site::new();
    site.file("work/wide.png", &png(9000, 10));
    let done = read(&site, "wide.png").await;
    assert!(done.error);
    assert!(done.images.is_empty());
    assert_eq!(
        site.call("read", serde_json::json!({"file_path": "wide.png"}))
            .await
            .1,
        "\"wide.png\" is 9000×10 pixels, too large to view. Images must be at most 8000 pixels on each side. Make a smaller copy with a command and read that.\n"
    );
    let said = done.human.expect("有说法");
    assert_eq!(said.key, "software/basesystem/read/image-too-wide");
    assert_eq!(
        (
            said.fields["width"].as_str(),
            said.fields["height"].as_str()
        ),
        ("9000", "10")
    );
    assert_eq!(
        done.effects,
        [read_effect(&site, "wide.png", &png(9000, 10))]
    );
    // 每边正好 8000 的照读；高过了也拦。
    site.file("work/square.png", &png(8000, 8000));
    assert_eq!(read(&site, "square.png").await.images.len(), 1);
    site.file("work/tall.png", &png(10, 8001));
    assert!(read(&site, "tall.png").await.error);
}

#[tokio::test]
async fn an_image_that_cannot_be_measured_is_binary() {
    let site = Site::new();
    let broken = b"\x89PNG\r\n\x1a\nnot really".to_vec();
    site.file("work/broken.png", &broken);
    let done = read(&site, "broken.png").await;
    assert!(done.error);
    assert!(done.images.is_empty());
    assert_eq!(
        site.call("read", serde_json::json!({"file_path": "broken.png"}))
            .await,
        (true, "\"broken.png\" is a binary file.\n".to_string())
    );
    assert_eq!(
        done.human.expect("有说法").key,
        "software/basesystem/read/binary"
    );
    assert_eq!(done.effects, [read_effect(&site, "broken.png", &broken)]);
}

#[test]
fn the_description_says_images_are_read_too() {
    assert!(
        tool("read").spec().description.starts_with(
            "Read a text file or an image (PNG, JPEG, GIF, WebP), or list a directory."
        )
    );
}

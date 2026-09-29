use miyu_kernel::id::MediaType;
use miyu_tool::picture::{MAX_BYTES, MAX_SIDE};

use super::{Kind, TooBig, kind};

/// 一张 `width` × `height` 的 PNG：签名和 IHDR 就够量宽高；`pad` 个字节接在后面，凑大小。
fn png(width: u32, height: u32, pad: usize) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes.resize(bytes.len() + pad, 0);
    bytes
}

fn media(text: &str) -> MediaType {
    MediaType::parse(text).unwrap()
}

fn file(media_type: &str) -> Result<Kind, TooBig> {
    Ok(Kind::File {
        media_type: media(media_type),
    })
}

#[test]
fn a_measurable_picture_is_an_image_whatever_the_head_says() {
    let image = Ok(Kind::Image {
        media_type: media("image/png"),
        width: 800,
        height: 600,
    });
    assert_eq!(kind(&png(800, 600, 0), None), image);
    assert_eq!(
        kind(&png(800, 600, 0), Some(media("image/jpeg"))),
        image,
        "头写的不算"
    );
    // 开头像 PNG、量不出宽高的，当文件。
    assert_eq!(
        kind(b"\x89PNG\r\n\x1a\n\x00\x00", None),
        file("application/octet-stream")
    );
}

#[test]
fn pictures_over_the_limits_are_too_big_and_the_limits_themselves_fit() {
    assert!(kind(&png(MAX_SIDE, MAX_SIDE, 0), None).is_ok());
    assert_eq!(kind(&png(MAX_SIDE + 1, 10, 0), None), Err(TooBig));
    assert_eq!(kind(&png(10, MAX_SIDE + 1, 0), None), Err(TooBig));
    let header = png(1, 1, 0).len();
    let exact = MAX_BYTES as usize - header;
    assert!(kind(&png(1, 1, exact), None).is_ok(), "正好 5 MiB 的收");
    assert_eq!(kind(&png(1, 1, exact + 1), None), Err(TooBig));
}

#[test]
fn a_pdf_is_known_by_its_first_bytes() {
    let pdf = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n";
    assert_eq!(kind(pdf, None), file("application/pdf"));
    assert_eq!(
        kind(pdf, Some(media("application/octet-stream"))),
        file("application/pdf"),
        "头写的不算"
    );
}

#[test]
fn other_files_take_what_the_head_said_unless_it_claims_pdf_or_image() {
    assert_eq!(
        kind(b"a,b\n1,2\n", Some(media("text/csv"))),
        file("text/csv")
    );
    assert_eq!(kind(b"a,b\n", None), file("text/plain"), "文本");
    assert_eq!(kind(b"", None), file("text/plain"), "空的也是文本");
    assert_eq!(
        kind(b"\x00\x01", None),
        file("application/octet-stream"),
        "二进制"
    );
    assert_eq!(
        kind(b"caf\xE9", None),
        file("application/octet-stream"),
        "不是 UTF-8"
    );
    // 内容不是 PDF、不是图，头却写成了的，照内容认的写。
    assert_eq!(
        kind(b"hello", Some(media("application/pdf"))),
        file("text/plain")
    );
    assert_eq!(
        kind(b"\x00", Some(media("image/png"))),
        file("application/octet-stream")
    );
}

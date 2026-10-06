//! 分段、下载的名字（施工 W-10）。

use super::range::{Span, attachment, span};

#[test]
fn one_range_or_the_whole() {
    let part = |start, end| Span::Part { start, end };
    for (header, want) in [
        (None, Span::Whole),
        (Some("bytes=0-4"), part(0, 4)),
        (Some("BYTES=0-4"), part(0, 4)),
        (Some("bytes=5-"), part(5, 9)),
        (Some("bytes=-3"), part(7, 9)),
        (Some("bytes=-30"), part(0, 9)),
        (Some("bytes=8-100"), part(8, 9)),
        (Some("bytes=9-9"), part(9, 9)),
        (Some("bytes=10-"), Span::Beyond),
        (Some("bytes=10-12"), Span::Beyond),
        (Some("bytes=-0"), Span::Beyond),
        (Some("bytes=0-1,4-5"), Span::Whole),
        (Some("bytes=4-1"), Span::Whole),
        (Some("bytes=a-"), Span::Whole),
        (Some("bytes=+1-2"), Span::Whole),
        (Some("bytes=-"), Span::Whole),
        (Some("bytes 0-1"), Span::Whole),
        (Some("items=0-1"), Span::Whole),
        (Some("byte"), Span::Whole),
    ] {
        assert_eq!(span(header, 10), want, "{header:?}");
    }
    assert_eq!(span(Some("bytes=-5"), 0), Span::Beyond, "空的没有哪一段");
    assert_eq!(span(Some("bytes=0-"), 0), Span::Beyond);
}

#[test]
fn a_download_name_keeps_its_last_part_escaped() {
    assert_eq!(attachment(None), "attachment");
    assert_eq!(attachment(Some("a/b/")), "attachment", "最后一段是空的");
    assert_eq!(
        attachment(Some("/home/me/plain.txt")),
        "attachment; filename*=UTF-8''plain.txt"
    );
    assert_eq!(
        attachment(Some("C:\\x\\a b.txt")),
        "attachment; filename*=UTF-8''a%20b.txt"
    );
    assert_eq!(
        attachment(Some("报告;\"'%.pdf")),
        "attachment; filename*=UTF-8''%E6%8A%A5%E5%91%8A%3B%22%27%25.pdf"
    );
    assert_eq!(
        attachment(Some("a!#$&+-.^_`|~z")),
        "attachment; filename*=UTF-8''a!#$&+-.^_`|~z"
    );
}

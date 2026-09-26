//! SSE 分帧：三种换行、注释、几行 data、切在两片之间的换行和汉字、流断在半条上。

use super::*;

fn events(chunks: &[&[u8]]) -> Vec<Event> {
    let mut sse = Sse::new();
    let mut out = Vec::new();
    for chunk in chunks {
        out.extend(sse.feed(chunk));
    }
    out.extend(sse.finish());
    out
}

fn data(texts: &[&str]) -> Vec<Event> {
    texts
        .iter()
        .map(|text| Event {
            event: None,
            data: (*text).to_string(),
        })
        .collect()
}

#[test]
fn three_kinds_of_newline() {
    for stream in [
        &b"data: a\n\ndata: b\n\n"[..],
        b"data: a\r\n\r\ndata: b\r\n\r\n",
        b"data: a\r\rdata: b\r\r",
    ] {
        assert_eq!(events(&[stream]), data(&["a", "b"]), "{stream:?}");
    }
}

#[test]
fn a_crlf_cut_in_two_is_one_newline() {
    // \r 在上一片末尾、\n 在下一片开头：还是一个换行，不多出一个空行把事件提前交出来。
    let got = events(&[b"data: a\r", b"\ndata: b\r\n\r\n"]);
    assert_eq!(got, data(&["a\nb"]));
}

#[test]
fn several_data_lines_and_comments() {
    let got = events(&[b": keep-alive\ndata: first\ndata:second\ndata\n\n"]);
    // 冒号后面的一个空格去掉，没有空格的照样；只有字段名的 data 是空的一行。
    assert_eq!(got, data(&["first\nsecond\n"]));
}

#[test]
fn only_comments_give_nothing() {
    assert!(events(&[b": ping\n\n: ping\n\n"]).is_empty());
}

#[test]
fn the_event_name_is_kept() {
    let got = events(&[b"event: error\ndata: {}\n\ndata: x\n\n"]);
    assert_eq!(
        got,
        [
            Event {
                event: Some("error".to_string()),
                data: "{}".to_string()
            },
            Event {
                event: None,
                data: "x".to_string()
            },
        ]
    );
}

#[test]
fn a_character_cut_in_two_is_not_broken() {
    let stream = "data: 你好\n\n".as_bytes();
    // 「你」的三个字节切在两片之间。
    let got = events(&[&stream[..7], &stream[7..]]);
    assert_eq!(got, data(&["你好"]));
}

#[test]
fn a_stream_cut_off_mid_event_still_gives_it() {
    assert_eq!(events(&[b"data: a\n\ndata: b"]), data(&["a", "b"]));
    assert_eq!(events(&[b"data: a\n"]), data(&["a"]));
}

#[test]
fn every_cut_gives_the_same_events() {
    let stream = "data: {\"a\":1}\r\n\r\n: x\ndata: 汉字\r\ndata: 二\r\r".as_bytes();
    let whole = events(&[stream]);
    for cut in 0..=stream.len() {
        assert_eq!(
            events(&[&stream[..cut], &stream[cut..]]),
            whole,
            "切在 {cut}"
        );
    }
    let bytes: Vec<&[u8]> = stream.chunks(1).collect();
    assert_eq!(events(&bytes), whole);
}

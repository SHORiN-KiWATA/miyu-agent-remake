//! 从记下的位置读起（施工 3-8 七补，`read_marked`）：会话列表的索引记着一行照到日志的哪里，列会话时只读多出来的那一截；
//! 对不上的（比记的短了、段没了、不在一行的开头）交回空的，调的一方整份重读。

use super::*;

/// 读 `dir`，从 `from` 起：交回读到的序号、读到了哪里。
fn marked(dir: &Path, from: Option<&Mark>) -> (Vec<u64>, Option<Mark>) {
    let mut seqs = Vec::new();
    let end = read_marked(dir, from, |events| {
        seqs.extend(events.iter().map(|event| event.seq.get()));
        true
    })
    .unwrap();
    (seqs, end)
}

#[test]
fn reading_from_the_start_ends_where_the_writer_is() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).unwrap();
    log.append(&said_range(1, 3)).unwrap();
    let (seqs, end) = marked(&dir, None);
    assert_eq!(seqs, [1, 2, 3]);
    assert_eq!(end, Some(log.mark()), "读到的位置就是写的一方记的");
}

#[test]
fn only_what_came_after_the_mark_is_read() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).unwrap();
    log.append(&said_range(1, 3)).unwrap();
    let mark = log.mark();
    assert_eq!(
        marked(&dir, Some(&mark)),
        (vec![], Some(mark)),
        "没有多出来的"
    );
    log.append(&said_range(4, 5)).unwrap();
    let (seqs, end) = marked(&dir, Some(&mark));
    assert_eq!(seqs, [4, 5], "只读多出来的那一截");
    assert_eq!(end, Some(log.mark()));
}

#[test]
fn reading_on_crosses_into_later_segments() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    // 一段写满一批就换段。
    let mut log = SessionLog::create(&dir, 1).unwrap();
    log.append(&said_range(1, 2)).unwrap();
    let mark = log.mark();
    log.append(&said_range(3, 3)).unwrap();
    log.append(&said_range(4, 5)).unwrap();
    assert_eq!(segment_names(&dir).len(), 3);
    let (seqs, end) = marked(&dir, Some(&mark));
    assert_eq!(seqs, [3, 4, 5]);
    assert_eq!(end, Some(log.mark()));
    assert_eq!(log.mark().segment, 4, "最后一段叫 4");
}

#[test]
fn a_reopened_log_knows_where_it_is() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create(&dir, 1).unwrap();
    log.append(&said_range(1, 2)).unwrap();
    log.append(&said_range(3, 3)).unwrap();
    drop(log);
    let (mut log, _) = SessionLog::open(&dir, 1).unwrap();
    assert_eq!(
        Some(log.mark()),
        marked(&dir, None).1,
        "载入以后照旧知道写在哪一段"
    );
    log.append(&said_range(4, 4)).unwrap();
    assert_eq!(Some(log.mark()), marked(&dir, None).1);
}

#[test]
fn a_half_line_at_the_end_is_left_for_later() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).unwrap();
    log.append(&said_range(1, 2)).unwrap();
    let mark = log.mark();
    let path = dir.join("000000000001.jsonl");
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str(&line_of(&said(3)));
    text.push_str(r#"{"seq":4,"#);
    fs::write(&path, &text).unwrap();
    let (seqs, end) = marked(&dir, Some(&mark));
    assert_eq!(seqs, [3]);
    let end = end.unwrap();
    assert_eq!(end.next.get(), 4);
    assert_eq!(
        end.bytes,
        (text.len() - r#"{"seq":4,"#.len()) as u64,
        "照到整行末尾"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), text, "一个字节不动");
}

#[test]
fn a_mark_that_does_not_line_up_gives_nothing() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).unwrap();
    log.append(&said_range(1, 3)).unwrap();
    let mark = log.mark();
    let beyond = Mark {
        bytes: mark.bytes + 1,
        ..mark
    };
    assert_eq!(marked(&dir, Some(&beyond)), (vec![], None), "比记的短了");
    let inside = Mark {
        bytes: mark.bytes - 3,
        ..mark
    };
    assert_eq!(
        marked(&dir, Some(&inside)),
        (vec![], None),
        "不在一行的开头"
    );
    let gone = Mark { segment: 7, ..mark };
    assert_eq!(
        marked(&dir, Some(&gone)),
        (vec![], None),
        "记的那一段没有了"
    );
}

#[test]
fn a_mark_whose_next_seq_is_off_is_broken() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).unwrap();
    log.append(&said_range(1, 2)).unwrap();
    let mark = log.mark();
    log.append(&said_range(3, 3)).unwrap();
    let off = Mark {
        next: Seq::new(9).unwrap(),
        ..mark
    };
    let read = read_marked(&dir, Some(&off), |_| true);
    assert!(
        matches!(read, Err(OpenError::Broken { .. })),
        "序号接不上报坏了：{read:?}"
    );
}

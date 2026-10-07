//! 会话列表的索引（施工 3-8 七补）：新建、会话一批批往上盖、照到的位置对不上的不盖、列会话写回只换没变过的那一行、删行；
//! 读不了、坏了、版本不对的删掉重建。都在临时目录里。

use std::fs;

use rusqlite::Connection;

use super::*;
use crate::test_support::Scratch;

const ID: &str = "0192f3a0-1111-7abc-8def-001122334455";

fn id() -> SessionId {
    SessionId::parse(ID).unwrap()
}

fn event(line: &str) -> Event {
    Event::from_line(line).unwrap()
}

fn created() -> Event {
    event(
        r#"{"seq":1,"at":"2026-09-25T07:00:00.000Z","kind":"session.created","by":{"kind":"person","account":"alice"},"cause":"c1","body":{"owner":"alice","venue":"local","policy":"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894","permission":{"level":"workspace","read_only":false},"oneshot":true,"cwd":"~/a"}}"#,
    )
}

fn renamed(seq: u64, title: &str, pinned: bool) -> Event {
    event(&format!(
        r#"{{"seq":{seq},"at":"2026-09-25T08:00:00.000Z","kind":"session.meta_changed","by":{{"kind":"person","account":"alice"}},"body":{{"title":"{title}","pinned":{pinned}}}}}"#
    ))
}

fn moved(seq: u64, cwd: &str) -> Event {
    event(&format!(
        r#"{{"seq":{seq},"at":"2026-09-25T09:00:00.000Z","kind":"turn.started","turn":{seq},"by":{{"kind":"kernel"}},"body":{{"cwd":"{cwd}"}}}}"#
    ))
}

fn mark(bytes: u64, next: u64) -> Mark {
    Mark {
        segment: 1,
        bytes,
        next: Seq::new(next).unwrap(),
    }
}

fn open(scratch: &Scratch) -> (SessionIndex, Opened) {
    SessionIndex::open(&scratch.path().join("index").join(FILE))
}

fn row(index: &SessionIndex) -> Option<Row> {
    index.rows().unwrap().remove(&id())
}

#[test]
fn a_new_index_is_created_empty_and_kept_next_time() {
    let scratch = Scratch::new();
    let (index, opened) = open(&scratch);
    assert!(matches!(opened, Opened::Created), "{opened:?}");
    assert!(index.rows().unwrap().is_empty());
    drop(index);
    let (_, opened) = open(&scratch);
    assert!(matches!(opened, Opened::Kept), "{opened:?}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(scratch.path().join("index"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }
}

#[test]
fn batches_are_laid_on_the_row_one_after_another() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    assert!(
        index
            .advance(&id(), &mark(0, 1), &[created()], &mark(100, 2))
            .unwrap()
    );
    let first = row(&index).unwrap();
    assert_eq!(first.owner.as_str(), "alice");
    assert!(first.oneshot);
    assert_eq!(first.parent, None);
    assert_eq!(first.cwd.as_deref(), Some("~/a"));
    assert_eq!(first.title, "");
    assert_eq!(first.created, first.last_active);
    assert_eq!(first.mark, mark(100, 2));
    let batch = [renamed(2, "发版", true), moved(3, "~/b")];
    assert!(
        index
            .advance(&id(), &mark(100, 2), &batch, &mark(300, 4))
            .unwrap()
    );
    let got = row(&index).unwrap();
    assert_eq!((got.title.as_str(), got.pinned), ("发版", true));
    assert_eq!(got.cwd.as_deref(), Some("~/b"));
    assert_eq!(got.last_active.to_string(), "2026-09-25T09:00:00.000Z");
    assert_eq!(got.created, first.created, "造的时刻不变");
    assert_eq!(got.mark, mark(300, 4));
    // 只改标题的一条，置顶照旧；只改置顶的，标题照旧。
    let only_title = event(
        r#"{"seq":4,"at":"2026-09-25T10:00:00.000Z","kind":"session.meta_changed","by":{"kind":"person","account":"alice"},"body":{"title":"改名"}}"#,
    );
    let only_pin = event(
        r#"{"seq":5,"at":"2026-09-25T10:01:00.000Z","kind":"session.meta_changed","by":{"kind":"person","account":"alice"},"body":{"pinned":false}}"#,
    );
    index
        .advance(&id(), &mark(300, 4), &[only_title], &mark(400, 5))
        .unwrap();
    let got = row(&index).unwrap();
    assert_eq!((got.title.as_str(), got.pinned), ("改名", true));
    index
        .advance(&id(), &mark(400, 5), &[only_pin], &mark(500, 6))
        .unwrap();
    let got = row(&index).unwrap();
    assert_eq!((got.title.as_str(), got.pinned), ("改名", false));
}

#[test]
fn a_batch_that_does_not_follow_the_row_is_left_for_the_list() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    assert!(
        !index
            .advance(
                &id(),
                &mark(100, 2),
                &[renamed(2, "x", false)],
                &mark(200, 3)
            )
            .unwrap(),
        "没有这一行不起"
    );
    assert!(row(&index).is_none());
    index
        .advance(&id(), &mark(0, 1), &[created()], &mark(100, 2))
        .unwrap();
    // 中间一批没盖上：这一批照的是 200，表里照到 100。
    assert!(
        !index
            .advance(
                &id(),
                &mark(200, 3),
                &[renamed(3, "y", false)],
                &mark(300, 4)
            )
            .unwrap()
    );
    let got = row(&index).unwrap();
    assert_eq!(
        (got.title.as_str(), got.mark),
        ("", mark(100, 2)),
        "一格都不动"
    );
}

#[test]
fn put_only_fills_a_gap_or_replaces_an_unchanged_row() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    let mut listed = Row::new(id(), &created()).unwrap();
    listed.mark = mark(100, 2);
    assert!(index.put(&listed, None).unwrap(), "没有这一行，写上");
    let mut later = listed.clone();
    later.see(&renamed(2, "后来", false));
    later.mark = mark(200, 3);
    assert!(!index.put(&later, None).unwrap(), "已经有了，不当没有写");
    assert!(
        !index.put(&later, Some(&mark(50, 2))).unwrap(),
        "表里那一行已经变了，不换"
    );
    assert_eq!(row(&index).unwrap(), listed);
    assert!(index.put(&later, Some(&mark(100, 2))).unwrap());
    assert_eq!(row(&index).unwrap(), later);
}

#[test]
fn a_removed_row_is_gone_and_removing_nothing_is_fine() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    index
        .advance(&id(), &mark(0, 1), &[created()], &mark(100, 2))
        .unwrap();
    index.remove(&id()).unwrap();
    assert!(row(&index).is_none());
    index.remove(&id()).unwrap();
}

#[test]
fn garbage_is_deleted_and_rebuilt_empty() {
    let scratch = Scratch::new();
    let path = scratch.path().join("index").join(FILE);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, vec![b'x'; 4096]).unwrap();
    let (index, opened) = SessionIndex::open(&path);
    assert!(matches!(opened, Opened::Rebuilt(_)), "{opened:?}");
    assert!(index.rows().unwrap().is_empty());
    index
        .advance(&id(), &mark(0, 1), &[created()], &mark(100, 2))
        .unwrap();
    assert!(row(&index).is_some(), "重建的用得了");
}

#[test]
fn another_version_is_deleted_and_rebuilt_empty() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    index
        .advance(&id(), &mark(0, 1), &[created()], &mark(100, 2))
        .unwrap();
    drop(index);
    let path = scratch.path().join("index").join(FILE);
    Connection::open(&path)
        .unwrap()
        .pragma_update(None, "user_version", VERSION + 1)
        .unwrap();
    let (index, opened) = SessionIndex::open(&path);
    assert!(
        matches!(opened, Opened::Rebuilt(IndexError::Bad(_))),
        "{opened:?}"
    );
    assert!(index.rows().unwrap().is_empty(), "旧的行不留");
}

#[test]
fn an_unreadable_row_is_an_error_and_reset_empties_the_index() {
    let scratch = Scratch::new();
    let (index, _) = open(&scratch);
    index
        .advance(&id(), &mark(0, 1), &[created()], &mark(100, 2))
        .unwrap();
    let path = scratch.path().join("index").join(FILE);
    Connection::open(&path)
        .unwrap()
        .execute("UPDATE sessions SET owner = 'Not An Account'", [])
        .unwrap();
    assert!(matches!(index.rows(), Err(IndexError::Bad(_))));
    index.reset().unwrap();
    assert!(index.rows().unwrap().is_empty());
}

fn said(seq: u64, blocks: &str) -> Event {
    event(&format!(
        r#"{{"seq":{seq},"at":"2026-09-25T09:00:00.000Z","kind":"message.user","by":{{"kind":"person","account":"alice"}},"cause":"c{seq}","body":{{"blocks":{blocks}}}}}"#
    ))
}

/// 第一句话的第一行（施工 9-5）：跳过开头的空行、去掉前后空白、截到 50 个字；没有文字的不算，第二句盖不过第一句。
#[test]
fn the_preview_is_the_first_line_of_the_first_words() {
    let mut row = Row::new(id(), &created()).unwrap();
    assert_eq!(row.preview, "");
    row.see(&said(2, r#"[{"type":"file","blob":"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894","name":"a.pdf","media_type":"application/pdf"}]"#));
    assert_eq!(row.preview, "", "只有附件的不算");
    let long = "长".repeat(60);
    row.see(&said(
        3,
        &format!(r#"[{{"type":"text","text":"\n  {long}  \n第二行"}}]"#),
    ));
    assert_eq!(row.preview, "长".repeat(50));
    row.see(&said(4, r#"[{"type":"text","text":"后来的"}]"#));
    assert_eq!(row.preview, "长".repeat(50), "第二句盖不过第一句");
}

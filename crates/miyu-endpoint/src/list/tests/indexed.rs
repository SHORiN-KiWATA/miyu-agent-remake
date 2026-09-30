//! 读索引的和整份读的一字不差（施工 3-8 七补）：同一批会话，没有索引、第一次（照日志填）、第二次（照索引读）列出来的一样；
//! 索引没看见的几条（崩在更新之前、手动加的）照日志补上；日志比记的短了、段对不上的整份重读；索引没有、坏了、版本不对，
//! 起来时删掉重建；用着用着读出坏了的，也删掉重建，这一次照样列得对。记的运行日志另见 `tests/index_log.rs`：接住运行日志要
//! 装全局的订阅者，一个测试程序只能装一次。

use std::fs;
use std::path::PathBuf;

use super::*;

const C: &str = "0192f3a0-3333-7abc-8def-001122334455";
const D: &str = "0192f3a0-4444-7abc-8def-001122334455";
const E: &str = "0192f3a0-5555-7abc-8def-001122334455";
const F: &str = "0192f3a0-6666-7abc-8def-001122334455";
const G: &str = "0192f3a0-7777-7abc-8def-001122334455";
const H: &str = "0192f3a0-8888-7abc-8def-001122334455";

/// D 的末尾有第 3 条的前这么多个字节：半行。
const HALF: usize = 10;

fn at(minute: u64) -> String {
    format!("2026-09-25T07:{minute:02}:00.000Z")
}

fn created(oneshot: bool, parent: Option<&str>) -> String {
    let parent = parent.map_or(String::new(), |parent| {
        format!(r#","parent":"{parent}","depth":1"#)
    });
    format!(
        r#"{{"seq":1,"at":"{}","kind":"session.created","by":{{"kind":"person","account":"alice"}},"cause":"c1","body":{{"owner":"alice","venue":"local","policy":"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894","permission":{{"level":"workspace","read_only":false}},"oneshot":{oneshot},"cwd":"~/a"{parent}}}}}"#,
        at(0)
    )
}

fn meta(seq: u64, title: Option<&str>, pinned: Option<bool>) -> String {
    let mut body = serde_json::Map::new();
    if let Some(title) = title {
        body.insert("title".into(), json!(title));
    }
    if let Some(pinned) = pinned {
        body.insert("pinned".into(), json!(pinned));
    }
    json!({"seq": seq, "at": at(seq), "kind": "session.meta_changed", "by": {"kind": "person", "account": "alice"}, "body": body}).to_string()
}

fn turn(seq: u64, cwd: Option<&str>) -> String {
    let body = cwd.map_or_else(|| json!({}), |cwd| json!({"cwd": cwd}));
    json!({"seq": seq, "at": at(seq), "kind": "turn.started", "turn": seq, "by": {"kind": "kernel"}, "cause": format!("c{seq}"), "body": body}).to_string()
}

fn said(seq: u64) -> String {
    json!({"seq": seq, "at": at(seq), "kind": "message.user", "by": {"kind": "person", "account": "alice"}, "body": {"blocks": [{"type": "text", "text": format!("第 {seq} 句")}]}}).to_string()
}

/// 这些行拼成一段：一行一个 `\n`。
fn lines(lines: &[String]) -> String {
    lines.iter().map(|line| format!("{line}\n")).collect()
}

/// 一批各种各样的会话：没记过工作目录的、改过名置顶又去掉标题的、两段的、末尾有半行的、日志坏了的、第一条不是
/// `session.created` 的、空目录、一次性的子会话。
fn varied() -> Site {
    let site = Site::new(&[(A, &[OLD])]);
    let b = [
        created(false, None),
        meta(2, Some("发版"), Some(true)),
        turn(3, Some("~/b")),
        said(4),
        meta(5, Some(""), None),
        turn(6, None),
    ];
    write(&site, B, "000000000001.jsonl", &lines(&b));
    write(
        &site,
        C,
        "000000000001.jsonl",
        &lines(&[created(false, None), said(2)]),
    );
    write(
        &site,
        C,
        "000000000003.jsonl",
        &lines(&[meta(3, Some("两段"), None), turn(4, Some("~/c"))]),
    );
    write(
        &site,
        D,
        "000000000001.jsonl",
        &(lines(&[created(false, None), said(2)]) + &said(3)[..HALF]),
    );
    let broken = lines(&[created(false, None), meta(2, Some("坏"), None)]) + "not json\n";
    write(&site, E, "000000000001.jsonl", &broken);
    write(&site, F, "000000000001.jsonl", &lines(&[said(1)]));
    fs::create_dir_all(dir(&site, G)).expect("建得了");
    write(
        &site,
        H,
        "000000000001.jsonl",
        &lines(&[created(true, Some(B)), turn(2, Some("~/h"))]),
    );
    site
}

fn dir(site: &Site, id: &str) -> PathBuf {
    site.1
        .session_dir(&alice(), &SessionId::parse(id).expect("合写法"))
}

fn write(site: &Site, id: &str, segment: &str, text: &str) {
    let dir = dir(site, id);
    fs::create_dir_all(&dir).expect("建得了");
    fs::write(dir.join(segment), text).expect("写得进");
}

fn append(site: &Site, id: &str, segment: &str, text: &str) {
    let path = dir(site, id).join(segment);
    let mut old = fs::read_to_string(&path).expect("读得了");
    old.push_str(text);
    fs::write(path, old).expect("写得进");
}

fn index_path(site: &Site) -> PathBuf {
    site.1.index(&alice()).join(FILE)
}

/// 列一次，写成 `session.list` 的回应那样；`oneshot` 只要一次性的。B 当忙着。
fn listing(site: &Site, index: Option<&SessionIndex>, oneshot: bool) -> Value {
    let busy = BTreeSet::from([SessionId::parse(B).expect("合写法")]);
    let pick = |row: &Row| !oneshot || row.oneshot;
    let listed = scan(
        &site.1,
        &alice(),
        index,
        &busy,
        pick,
        None,
        &Stop::default(),
    )
    .expect("读得了");
    Value::Array(listed.iter().map(Listed::to_json).collect())
}

fn row(index: &SessionIndex, id: &str) -> Option<Row> {
    index
        .rows()
        .expect("读得了")
        .remove(&SessionId::parse(id).expect("合写法"))
}

#[test]
fn reading_the_index_gives_what_reading_every_log_gives() {
    let site = varied();
    let whole = listing(&site, None, false);
    assert_eq!(
        whole.as_array().map(Vec::len),
        Some(6),
        "F、G 不列：{whole}"
    );
    let (index, _) = SessionIndex::open(&index_path(&site));
    assert_eq!(
        listing(&site, Some(&index), true),
        listing(&site, None, true),
        "只要一次性的"
    );
    for pass in ["照日志填", "照索引读", "再读一次"] {
        assert_eq!(listing(&site, Some(&index), false), whole, "{pass}");
    }
    let rows = index.rows().expect("读得了");
    let ids: Vec<&str> = rows.keys().map(SessionId::as_str).collect();
    assert_eq!(
        ids,
        [A, B, C, D, H],
        "日志坏了的不写进索引（照坏的那一段以前的算，下次还整份读）"
    );
    let d = dir(&site, D).join("000000000001.jsonl");
    let written = fs::read_to_string(&d).expect("读得了");
    assert_eq!(
        rows[&SessionId::parse(D).expect("合写法")].mark.bytes,
        (written.len() - HALF) as u64,
        "照到整行末尾"
    );
    assert_eq!(
        rows[&SessionId::parse(C).expect("合写法")].mark.segment,
        3,
        "照到最后一段"
    );
}

#[test]
fn what_the_index_did_not_see_is_read_on_top() {
    let site = varied();
    let (index, _) = SessionIndex::open(&index_path(&site));
    listing(&site, Some(&index), false);
    // 会话写了几条、没来得及更新索引就崩了：半行接着写完，再加两条。
    append(
        &site,
        D,
        "000000000001.jsonl",
        &format!("{}\n", &said(3)[HALF..]),
    );
    append(
        &site,
        D,
        "000000000001.jsonl",
        &lines(&[meta(4, Some("后来"), Some(true)), turn(5, Some("~/d"))]),
    );
    // 另一个换了段。
    write(
        &site,
        C,
        "000000000005.jsonl",
        &lines(&[meta(5, None, Some(true))]),
    );
    assert_eq!(
        listing(&site, Some(&index), false),
        listing(&site, None, false)
    );
    let d = row(&index, D).expect("有这一行");
    assert_eq!(
        (d.title.as_str(), d.pinned, d.cwd.as_deref()),
        ("后来", true, Some("~/d")),
        "补好写回去"
    );
    let log = fs::metadata(dir(&site, D).join("000000000001.jsonl"))
        .expect("在")
        .len();
    assert_eq!(d.mark.bytes, log);
    assert_eq!(d.mark.next.get(), 6);
    assert_eq!(row(&index, C).expect("有这一行").mark.segment, 5);
}

#[test]
fn a_log_shorter_than_the_row_or_off_the_line_is_read_whole() {
    let site = varied();
    let (index, _) = SessionIndex::open(&index_path(&site));
    listing(&site, Some(&index), false);
    // 手动截掉最后两条：比记的短了。
    write(
        &site,
        B,
        "000000000001.jsonl",
        &lines(&[
            created(false, None),
            meta(2, Some("发版"), Some(true)),
            turn(3, Some("~/b")),
            said(4),
        ]),
    );
    // 换成更长的一份，记的位置落在一行中间。
    let a = r#"{"seq":1,"at":"2026-09-25T07:00:00.000Z","kind":"session.created","by":{"kind":"person","account":"alice"},"cause":"c1","body":{"owner":"alice","venue":"local","policy":"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894","permission":{"level":"workspace","read_only":false},"cwd":"~/rewritten"}}"#;
    write(
        &site,
        A,
        "000000000001.jsonl",
        &lines(&[a.to_string(), meta(2, Some("重写"), None)]),
    );
    // 记的那一段没了。
    fs::remove_file(dir(&site, C).join("000000000003.jsonl")).expect("删得掉");
    let whole = listing(&site, None, false);
    assert_eq!(listing(&site, Some(&index), false), whole);
    assert_eq!(
        listing(&site, Some(&index), false),
        whole,
        "重读的写回去了，下一次照它"
    );
    let b = row(&index, B).expect("有这一行");
    assert_eq!((b.title.as_str(), b.mark.next.get()), ("发版", 5));
    assert_eq!(row(&index, A).expect("有这一行").title, "重写");
    assert_eq!(row(&index, C).expect("有这一行").mark.segment, 1);
}

/// 怎么弄坏索引：叫什么，弄坏它。
type Breakage = (&'static str, fn(&Site));

#[test]
fn a_missing_broken_or_old_index_is_rebuilt_from_the_logs() {
    let breakages: [Breakage; 3] = [
        ("没有", |_| {}),
        ("坏了", |site| {
            let path = index_path(site);
            fs::create_dir_all(path.parent().expect("有上一层")).expect("建得了");
            fs::write(path, vec![b'x'; 4096]).expect("写得进");
        }),
        ("版本不对", |site| {
            drop(SessionIndex::open(&index_path(site)));
            rusqlite::Connection::open(index_path(site))
                .expect("打得开")
                .pragma_update(None, "user_version", 99)
                .expect("改得了");
        }),
    ];
    for (said, broken) in breakages {
        let site = varied();
        broken(&site);
        let whole = listing(&site, None, false);
        let index = open_index(&site.1, &alice());
        assert!(
            index.rows().expect("读得了").is_empty(),
            "{said}：换了一份空的"
        );
        let first = listing(&site, Some(&index), false);
        let second = listing(&site, Some(&index), false);
        assert_eq!((first, second), (whole.clone(), whole), "{said}");
        assert_eq!(
            index.rows().expect("读得了").len(),
            5,
            "照日志填好了：{said}"
        );
    }
}

#[test]
fn an_index_found_broken_while_listing_is_rebuilt() {
    let site = varied();
    let index = open_index(&site.1, &alice());
    let whole = listing(&site, Some(&index), false);
    rusqlite::Connection::open(index_path(&site))
        .expect("打得开")
        .execute("UPDATE sessions SET owner = 'Not An Account'", [])
        .expect("改得了");
    assert_eq!(listing(&site, Some(&index), false), whole);
    assert_eq!(
        index.rows().expect("重建了，读得了").len(),
        5,
        "这一次读的写进了新的里"
    );
}

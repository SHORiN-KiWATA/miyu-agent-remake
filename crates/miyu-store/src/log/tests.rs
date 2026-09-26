//! 会话日志的测试：写几批、关掉再打开；换段；崩了留下的半行；坏了的几种；空的最后一段；没有这个
//! 会话；真会话来回一趟。都在临时目录里。

use std::collections::BTreeMap;
use std::fs;

use miyu_kernel::assemble::Assembler;
use miyu_kernel::facts::{Environment, FactTemplates};
use miyu_kernel::history::History;
use miyu_kernel::request::Request;
use miyu_kernel::session::{Policy, Session};
use miyu_kernel::testkit::{Line, Play, Stage};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_kernel::tool::{Access, ToolRule, ToolTextSources, ToolTexts};

use super::*;
use crate::test_support::Scratch;

/// 第 `n` 号事件：alice 说的第 `n` 句。日志这一层不管事件之间的规矩，只管序号和能不能读。
fn said(n: u64) -> Event {
    Event::from_line(&format!(
        r#"{{"seq":{n},"at":"2026-09-25T07:00:00.000Z","kind":"message.user","by":{{"kind":"person","account":"alice"}},"body":{{"blocks":[{{"type":"text","text":"第 {n} 句"}}]}}}}"#
    ))
    .unwrap()
}

/// 第 `from` 到第 `to` 号。
fn said_range(from: u64, to: u64) -> Vec<Event> {
    (from..=to).map(said).collect()
}

/// 一行事件写进文件是什么样：它的一行加换行。
fn line_of(event: &Event) -> String {
    event.to_line() + "\n"
}

fn dir(scratch: &Scratch) -> PathBuf {
    scratch.path().join("sessions").join("s")
}

/// 目录里的段，照名字排。
fn segment_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn what_is_written_comes_back() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).unwrap();
    log.append(&said_range(1, 3)).unwrap();
    log.append(&said_range(4, 4)).unwrap();
    log.append(&[]).unwrap();
    assert_eq!(log.next_seq().get(), 5);
    drop(log);
    let (mut log, events) = SessionLog::open(&dir, SEGMENT_LIMIT).unwrap();
    assert_eq!(events, said_range(1, 4));
    assert_eq!(log.next_seq().get(), 5);
    // 每行以 \n 结尾，没有 \r；接着写，序号接得上。
    log.append(&said_range(5, 6)).unwrap();
    let text = fs::read_to_string(dir.join("000000000001.jsonl")).unwrap();
    assert_eq!(
        text,
        said_range(1, 6).iter().map(line_of).collect::<String>()
    );
    assert!(!text.contains('\r'));
}

#[test]
fn a_full_segment_rolls_over_and_a_batch_is_not_split() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    // 上限比一行还短：每一批都另起一段，一批几行都在同一段里。
    let mut log = SessionLog::create(&dir, 10).unwrap();
    log.append(&said_range(1, 2)).unwrap();
    log.append(&said_range(3, 5)).unwrap();
    log.append(&said_range(6, 6)).unwrap();
    assert_eq!(
        segment_names(&dir),
        [
            "000000000001.jsonl",
            "000000000003.jsonl",
            "000000000006.jsonl"
        ]
    );
    let second = fs::read_to_string(dir.join("000000000003.jsonl")).unwrap();
    assert_eq!(
        second,
        said_range(3, 5).iter().map(line_of).collect::<String>()
    );
    drop(log);
    let (_, events) = SessionLog::open(&dir, 10).unwrap();
    assert_eq!(events, said_range(1, 6));
}

#[test]
fn a_half_written_last_line_is_cut_off() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).unwrap();
    log.append(&said_range(1, 2)).unwrap();
    drop(log);
    // 崩在写第 3 行的半中间。
    let path = dir.join("000000000001.jsonl");
    let whole = said_range(1, 2).iter().map(line_of).collect::<String>();
    let half = &line_of(&said(3))[..20];
    fs::write(&path, format!("{whole}{half}")).unwrap();
    let (mut log, events) = SessionLog::open(&dir, SEGMENT_LIMIT).unwrap();
    assert_eq!(events, said_range(1, 2));
    assert_eq!(fs::read_to_string(&path).unwrap(), whole, "半行截掉了");
    log.append(&said_range(3, 3)).unwrap();
    drop(log);
    let (_, events) = SessionLog::open(&dir, SEGMENT_LIMIT).unwrap();
    assert_eq!(events, said_range(1, 3));
}

/// 打开要报「坏了」：第 `line` 行，原因里有 `why`。
fn assert_broken(dir: &Path, segment: &str, line: usize, why: &str) {
    match SessionLog::open(dir, SEGMENT_LIMIT) {
        Err(OpenError::Broken {
            segment: at,
            line: got,
            why: said,
        }) => {
            assert_eq!(at, dir.join(segment));
            assert_eq!(got, line, "{said}");
            assert!(said.contains(why), "{said}");
        }
        other => panic!("应该报坏了：{other:?}"),
    }
}

#[test]
fn a_broken_log_is_reported_not_fixed() {
    // 中间一行读不出来。
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("000000000001.jsonl");
    let text = format!("{}not json\n{}", line_of(&said(1)), line_of(&said(2)));
    fs::write(&path, &text).unwrap();
    assert_broken(&dir, "000000000001.jsonl", 2, "读不出来");
    assert_eq!(fs::read_to_string(&path).unwrap(), text, "不自动修");
    // 序号接不上。
    let text = format!("{}{}", line_of(&said(1)), line_of(&said(3)));
    fs::write(&path, &text).unwrap();
    assert_broken(&dir, "000000000001.jsonl", 2, "序号应该是 2");
    // 段的名字和第一条对不上。
    fs::write(&path, line_of(&said(1))).unwrap();
    fs::write(dir.join("000000000005.jsonl"), line_of(&said(2))).unwrap();
    assert_broken(&dir, "000000000005.jsonl", 1, "这一段叫 5");
    // 不是最后一段的末尾有半行。
    fs::write(dir.join("000000000001.jsonl"), &line_of(&said(1))[..20]).unwrap();
    assert_broken(&dir, "000000000001.jsonl", 1, "后面还有段");
}

#[test]
fn an_empty_last_segment_is_written_into() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    // 刚建好第一段就崩了：一行都没有。
    drop(SessionLog::create(&dir, SEGMENT_LIMIT).unwrap());
    let (mut log, events) = SessionLog::open(&dir, SEGMENT_LIMIT).unwrap();
    assert!(events.is_empty());
    log.append(&said_range(1, 1)).unwrap();
    // 换段时建好新的一段就崩了。
    drop(log);
    fs::write(dir.join("000000000002.jsonl"), "").unwrap();
    let (mut log, events) = SessionLog::open(&dir, SEGMENT_LIMIT).unwrap();
    assert_eq!(events, said_range(1, 1));
    log.append(&said_range(2, 2)).unwrap();
    assert_eq!(
        fs::read_to_string(dir.join("000000000002.jsonl")).unwrap(),
        line_of(&said(2))
    );
    // 空的最后一段名字不对：报坏了。
    drop(log);
    fs::write(dir.join("000000000009.jsonl"), "").unwrap();
    assert_broken(&dir, "000000000009.jsonl", 1, "空的最后一段叫 9");
}

#[test]
fn no_session_no_log() {
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    assert!(matches!(
        SessionLog::open(&dir, SEGMENT_LIMIT),
        Err(OpenError::Missing(_))
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("notes.txt"), "不是段").unwrap();
    assert!(matches!(
        SessionLog::open(&dir, SEGMENT_LIMIT),
        Err(OpenError::Missing(_))
    ));
    // 已经有第一段的，不再新建，不覆盖。
    drop(SessionLog::create(&dir, SEGMENT_LIMIT).unwrap());
    assert!(SessionLog::create(&dir, SEGMENT_LIMIT).is_err());
}

#[test]
fn appending_out_of_order_is_refused() {
    let scratch = Scratch::new();
    let mut log = SessionLog::create(&dir(&scratch), SEGMENT_LIMIT).unwrap();
    let error = log.append(&said_range(2, 3)).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(log.next_seq(), Seq::FIRST, "什么都没写");
}

/// 替身用的组装：日志这一层不看请求，给一份空的。
struct Nothing;

impl Assembler for Nothing {
    fn assemble(&self, _history: &History) -> Request {
        Request {
            tools: Vec::new(),
            system: String::new(),
            messages: Vec::new(),
            stable: 0,
        }
    }
}

/// 替身用的策略：一件读的工具，句子短，一眼认得出。
fn policy() -> Policy {
    Policy {
        assembler: Box::new(Nothing),
        facts: FactTemplates::new(
            r#"<e t="{time}" d="{cwd}"/>"#,
            r#"<p l="{level}"/>"#,
            "<reply-cut/>",
        )
        .unwrap(),
        tools: BTreeMap::from([(
            "read".to_string(),
            ToolRule {
                access: Access::Read,
                parameters: serde_json::from_str(r#"{"type":"object"}"#).unwrap(),
            },
        )]),
        step_limit: None,
        tool_texts: ToolTexts::new(ToolTextSources {
            unknown: "no tool {name}",
            not_an_object: "bad args {name}",
            cancelled_before: "cancelled before",
            cancelled_running: "cancelled running",
            skipped: "skipped",
            read_only: "read only",
            denied: "denied",
            denied_with_reason: "denied: {reason}",
            unattended: "unattended",
            question_interrupted: "question interrupted",
            question_voided: "question voided",
            question_unattended: "question unattended",
            restarted: "restarted",
        })
        .unwrap(),
        attended: true,
        resumes: 3,
    }
}

fn environment() -> Environment {
    Environment {
        offset: UtcOffset::from_minutes(540).unwrap(),
        cwd: "~/src/miyu".to_string(),
    }
}

#[test]
fn a_real_session_goes_to_disk_and_loads_back() {
    // 替身跑一段真会话：调一次工具，说完。
    let start = Timestamp::parse("2026-09-25T07:00:00.000Z").unwrap();
    let mut stage = Stage::new(policy, environment(), start);
    stage.model([
        Line::calls("我看看。", &[("read", r#"{"path":"a"}"#)]),
        Line::says("好了。"),
    ]);
    stage.tools([Play::done("A")]);
    stage.say("看看 a");
    // 照三条一批写进日志，关掉再打开。
    let scratch = Scratch::new();
    let dir = dir(&scratch);
    let mut log = SessionLog::create(&dir, 300).unwrap();
    for batch in stage.log().chunks(3) {
        log.append(batch).unwrap();
    }
    drop(log);
    let (_, events) = SessionLog::open(&dir, 300).unwrap();
    assert_eq!(events, stage.log());
    assert!(segment_names(&dir).len() > 1, "上限调小了，要换过段");
    // 读回来的交给内核载入：过得了账本。
    let at = Timestamp::parse("2026-09-25T08:00:00.000Z").unwrap();
    let (_, actions) = Session::load(events, at, policy(), environment()).unwrap();
    assert!(actions.is_empty(), "走完了的会话，载入时什么都不补");
}

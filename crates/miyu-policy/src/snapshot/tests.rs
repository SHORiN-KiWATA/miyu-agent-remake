//! 快照的字节、读回来、造策略。随核心附带的字用仓库里出厂的那一份（编译时拿进来，不是读文件）。

use super::*;
use crate::compose::{PersonaTexts, Sources, compose};
use crate::test_support::*;

#[test]
fn the_engineer_system_is_the_one_sentence() {
    let snapshot = engineer();
    assert_eq!(snapshot.persona, "engineer");
    assert_eq!(snapshot.system, "You are a helpful software engineer.");
    assert_eq!(snapshot.step_limit, None);
    assert_eq!(snapshot.resumes, 3);
}

#[test]
fn the_same_sources_give_the_same_bytes_and_they_read_back() {
    let (one, two) = (engineer(), engineer());
    assert_eq!(one.to_bytes(), two.to_bytes());
    assert_eq!(one.hash(), two.hash());
    assert_eq!(one.hash(), ContentHash::of(&one.to_bytes()));
    assert_eq!(Snapshot::from_bytes(&one.to_bytes()), Ok(one.clone()));
    // 字段的先后就是字节里的先后，紧凑、不换行。
    let text = String::from_utf8(one.to_bytes()).unwrap();
    assert!(text.starts_with(r#"{"persona":"engineer","system":"You are a helpful software engineer.","core":{"checkpoint_open":"#), "{text}");
    assert!(
        text.ends_with(r#""step_limit":null,"attended":true,"resumes":3,"compaction":{"reserve_cap":20000,"margin":13000,"image":2000,"file":2000,"tail":16000,"rebuild":{"files":5,"file_tokens":5000,"total":50000,"min_window":32000,"candidates":10},"pause":{"failures":3,"turns":3,"refills":3}}}"#),
        "{text}"
    );
    // 改一个字，哈希就变了。
    let mut other = one.clone();
    other.system.push('!');
    assert_ne!(other.hash(), one.hash());
}

#[test]
fn broken_bytes_do_not_read_back() {
    let error = Snapshot::from_bytes(b"{\"persona\":1}").unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("policy snapshot not readable: "),
        "{error}"
    );
}

#[test]
fn the_policy_is_built_and_a_broken_template_is_named() {
    let snapshot = engineer();
    let policy = snapshot.policy().unwrap();
    assert!(policy.attended);
    assert_eq!(policy.resumes, 3);
    assert!(policy.tools.is_empty());
    assert!(snapshot.driver_texts().is_ok());
    let mut broken = snapshot.clone();
    broken.core.facts.env = "<env time=\"{time\"/>".to_string();
    let error = broken.policy().err().unwrap();
    assert!(
        matches!(
            error,
            BuildError::Texts {
                which: "fact templates",
                ..
            }
        ),
        "{error:?}"
    );
    let mut broken = snapshot.clone();
    broken.core.tool_results.unknown = "{nope".to_string();
    let error = broken.policy().err().unwrap();
    assert!(
        matches!(
            error,
            BuildError::Texts {
                which: "kernel's tool result texts",
                ..
            }
        ),
        "{error:?}"
    );
    let mut broken = snapshot;
    broken.core.drivers.file_omitted = "{".to_string();
    let error = broken.driver_texts().unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("bundled driver placeholders not usable: "),
        "{error}"
    );
}

#[test]
fn the_session_is_created_with_the_snapshot_hash() {
    let snapshot = engineer();
    let created = snapshot.session_created(
        AccountId::parse("local").unwrap(),
        VenueId::parse("local").unwrap(),
        Permission {
            level: miyu_kernel::event::Level::Workspace,
            read_only: false,
        },
    );
    assert_eq!(created.policy, snapshot.hash());
}

#[test]
fn the_switches_are_carried_as_given() {
    // 没人能确认的场所：拼的时候照给的记，造策略时照快照的带。
    let sources = Sources {
        core: core(),
        persona: PersonaTexts {
            persona: "x".to_string(),
        },
    };
    let unattended = compose("engineer", sources, false);
    assert!(!unattended.attended);
    assert!(!unattended.policy().unwrap().attended);
    // 步数上限照快照的带：不限的是不限，定了的是那个数。
    assert_eq!(engineer().policy().unwrap().step_limit, None);
    let mut limited = engineer();
    limited.step_limit = Some(5);
    assert_eq!(limited.policy().unwrap().step_limit, Some(5));
}

#[test]
fn each_driver_placeholder_is_its_own() {
    let texts = engineer().driver_texts().unwrap();
    let drivers = core().drivers;
    assert_eq!(texts.image_omitted(), drivers.image_omitted);
    assert_eq!(texts.no_output(), drivers.no_output);
    assert_eq!(texts.tool_attachments(), drivers.tool_attachments);
    assert_eq!(texts.tool_attachments_only(), drivers.tool_attachments_only);
    assert!(
        texts
            .file_omitted("a.pdf", "application/pdf")
            .contains("a.pdf")
    );
}

/// 压缩（施工 6-2 上）：出厂的快照带着压缩的数和摘要指令，造出的策略会主动压；以前造的快照没有这两格，读回来照旧，
/// 字节不变，造出的策略不主动压；缺一样也不压。
#[test]
fn compaction_comes_with_new_snapshots_and_old_ones_read_back_without_it() {
    let snapshot = engineer();
    let compaction = snapshot.policy().unwrap().compaction.unwrap();
    assert_eq!(
        (compaction.reserve_cap, compaction.margin),
        (20_000, 13_000)
    );
    assert_eq!(
        (compaction.price.image, compaction.price.file),
        (2000, 2000)
    );
    let mut old = snapshot.clone();
    old.compaction = None;
    old.core.compaction = None;
    let bytes = String::from_utf8(old.to_bytes()).unwrap();
    assert!(!bytes.contains("compaction"), "没有的不写：{bytes}");
    assert_eq!(
        Snapshot::from_bytes(old.to_bytes().as_slice()),
        Ok(old.clone())
    );
    assert!(old.policy().unwrap().compaction.is_none());
    let mut no_text = snapshot.clone();
    no_text.core.compaction = None;
    assert!(no_text.policy().unwrap().compaction.is_none());
    let mut no_numbers = snapshot;
    no_numbers.compaction = None;
    assert!(no_numbers.policy().unwrap().compaction.is_none());
}

/// 尾巴的上限（施工 6-2 下）：出厂 16000；6-2（上）造的快照里没有这一格，读成 16000。
#[test]
fn the_tail_is_16000_and_older_snapshots_read_it_so() {
    let snapshot = engineer();
    assert_eq!(snapshot.policy().unwrap().compaction.unwrap().tail, 16_000);
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let numbers = r#","tail":16000,"rebuild":{"files":5,"file_tokens":5000,"total":50000,"min_window":32000,"candidates":10},"pause":{"failures":3,"turns":3,"refills":3}}}"#;
    assert!(text.ends_with(numbers), "{text}");
    let older = text.replace(numbers, "}}");
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert_eq!(read.compaction.unwrap().tail, 16_000);
}

/// 压后重建（施工 6-5）：出厂的快照带着字和数，内核拿到几段的模板和重建的数，组装器拿到重读的文件那一块的头尾；以前
/// 造的快照里没有，读成没有，内核不写那几段、不重读，包装的结尾读成空的。
#[test]
fn rebuild_texts_and_numbers_go_in_and_older_snapshots_lack_them() {
    let snapshot = engineer();
    let policy = snapshot.policy().unwrap();
    assert!(policy.notes.is_some());
    let rebuild = policy.compaction.unwrap().rebuild.unwrap();
    assert_eq!(
        (rebuild.files, rebuild.file_tokens, rebuild.total),
        (5, 5_000, 50_000)
    );
    assert_eq!((rebuild.min_window, rebuild.candidates), (32_000, 10));
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let start = text.find(r#","rebuild":{"notes_files""#).unwrap();
    let close = r#""restored_close":"\n</file>\n"}"#;
    let end = start + text[start..].find(close).unwrap() + close.len();
    let older = text[..start].to_string() + &text[end..];
    let older = older.replace(r#","rebuild":{"files":5,"file_tokens":5000,"total":50000,"min_window":32000,"candidates":10},"pause":{"failures":3,"turns":3,"refills":3}"#, "");
    let older = older.replace(
        r#","checkpoint_end":"Carry on from where the summary leaves off, without redoing work it records as done.\n</conversation-checkpoint>\n""#,
        "",
    );
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert_eq!(read.core.checkpoint_end, "");
    let policy = read.policy().unwrap();
    assert!(policy.notes.is_none());
    assert!(policy.compaction.unwrap().rebuild.is_none());
    assert_eq!(read.to_bytes(), older.as_bytes(), "读进来再写出去一字不差");
    // 只有数、没有字的：也不重读。
    let only_numbers = text[..start].to_string() + &text[end..];
    let read = Snapshot::from_bytes(only_numbers.as_bytes()).unwrap();
    assert!(read.policy().unwrap().compaction.unwrap().rebuild.is_none());
}

/// 包装的结尾交给了组装器（施工 6-5）：出厂快照组装出来的检查点最后是规则那一句。
#[test]
fn the_checkpoint_ends_with_the_rule_from_the_snapshot() {
    use miyu_kernel::event::Event;
    use miyu_kernel::history::History;
    let policy = engineer().policy().unwrap();
    let mut history = History::default();
    history.append(
        Event::from_line(
            r#"{"seq":2,"at":"2026-09-29T05:00:00.000Z","kind":"context.compacted","by":{"kind":"kernel"},"body":{"upto":1,"summary":"S"}}"#,
        )
        .unwrap(),
    );
    let request = policy.assembler.assemble(&history);
    let text = format!("{:?}", request.messages);
    assert!(
        text.contains("S\\n</summary>\\nCarry on from where the summary leaves off"),
        "{text}"
    );
}

/// 熔断的数（施工 6-6 上）：出厂的快照带着 3、3、3，内核拿到；以前造的快照里没有，读成没有，不熔断，读进来再写出去一字
/// 不差。
#[test]
fn pause_numbers_go_in_and_older_snapshots_lack_them() {
    let snapshot = engineer();
    let pause = snapshot
        .policy()
        .unwrap()
        .compaction
        .unwrap()
        .pause
        .unwrap();
    assert_eq!((pause.failures, pause.turns, pause.refills), (3, 3, 3));
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let older = text.replace(r#","pause":{"failures":3,"turns":3,"refills":3}"#, "");
    assert_ne!(older, text);
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert!(read.policy().unwrap().compaction.unwrap().pause.is_none());
    assert_eq!(read.to_bytes(), older.as_bytes());
}

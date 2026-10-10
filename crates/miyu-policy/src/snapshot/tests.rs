//! 快照的字节、读回来、造策略。随核心附带的字用仓库里出厂的那一份（编译时拿进来，不是读文件）。会话编号的模板在
//! `facts.rs`。

mod drivers;
mod facts;
mod jobs;
mod recap;
mod title;
mod vision;

use super::*;
use crate::compose::{PersonaTexts, Sources, compose};
use crate::test_support::*;

#[test]
fn the_engineer_system_is_the_one_sentence() {
    let snapshot = engineer();
    assert_eq!(snapshot.persona.as_deref(), Some("engineer"));
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
    // 字段的先后就是字节里的先后，紧凑、不换行。人格的指纹排在最后（施工 P-1 再补）。
    let digest = serde_json::to_string(&one.persona_digest).unwrap();
    assert!(
        String::from_utf8(one.to_bytes())
            .unwrap()
            .ends_with(&format!(",\"persona_digest\":{digest}}}"))
    );
    let text = crate::test_support::text_without_digest(&one);
    assert!(text.starts_with(r#"{"persona":"engineer","system":"You are a helpful software engineer.","core":{"checkpoint_open":"#), "{text}");
    assert!(
        text.ends_with(r#""step_limit":null,"attended":true,"resumes":3,"compaction":{"reserve_cap":20000,"margin":13000,"line_percent":85,"margin_percent":5,"image":2000,"file":2000,"tail":16000,"lead":16000,"rebuild":{"files":5,"file_tokens":5000,"total":50000,"min_window":32000,"candidates":10},"pause":{"failures":3,"turns":3,"refills":3},"shorten":{"tries":3,"percent":20}},"jobs":{"report_chars":30000},"recap":{"turns":8,"tokens":8192},"title":{"tokens":1024,"chars":50,"tries":2},"peers":{"burst":5,"window":600,"unread":50,"watch_hours":12,"status_chars":200}}"#),
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
            examples: Vec::new(),
            reminders: String::new(),
        },
        reminder: Default::default(),
    };
    let unattended = compose(Some("engineer"), sources, false);
    assert!(!unattended.attended);
    assert!(!unattended.policy().unwrap().attended);
    // 步数上限照快照的带：不限的是不限，定了的是那个数。
    assert_eq!(engineer().policy().unwrap().step_limit, None);
    let mut limited = engineer();
    limited.step_limit = Some(5);
    assert_eq!(limited.policy().unwrap().step_limit, Some(5));
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
    let text = crate::test_support::text_without_digest(&snapshot)
        .replace(JOB_NUMBERS, "")
        .replace(RECAP_NUMBERS, "")
        .replace(TITLE_NUMBERS, "")
        .replace(PEER_NUMBERS, "");
    let numbers = r#","tail":16000,"lead":16000,"rebuild":{"files":5,"file_tokens":5000,"total":50000,"min_window":32000,"candidates":10},"pause":{"failures":3,"turns":3,"refills":3},"shorten":{"tries":3,"percent":20}}}"#;
    assert!(text.ends_with(numbers), "{text}");
    let older = text.replace(numbers, "}}");
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert_eq!(read.compaction.unwrap().tail, 16_000);
}

/// 提前量的上限（施工 6-11 上）：出厂 16000，排在尾巴后面；以前造的快照里没有这一格，读成 16000，内核拿到的照它。
#[test]
fn the_lead_is_16000_and_older_snapshots_read_it_so() {
    let mut snapshot = engineer();
    let text = crate::test_support::text_without_digest(&snapshot);
    let pair = r#""tail":16000,"lead":16000,"rebuild""#;
    assert!(text.contains(pair), "{text}");
    let older = Snapshot::from_bytes(text.replace(r#""lead":16000,"#, "").as_bytes()).unwrap();
    assert_eq!(older.compaction.unwrap().lead, 16_000);
    snapshot.compaction.as_mut().unwrap().lead = 7;
    assert_eq!(snapshot.policy().unwrap().compaction.unwrap().lead, 7);
}

/// 压缩线、余量各至多窗口的百分之几（施工 6-11 再补）：出厂 85、5，排在余量后面；以前造的快照里没有这两格，读成出厂的，
/// 内核拿到的照它。
#[test]
fn the_line_is_capped_at_85_percent_and_older_snapshots_read_it_so() {
    let mut snapshot = engineer();
    let text = crate::test_support::text_without_digest(&snapshot);
    let pair = r#""margin":13000,"line_percent":85,"margin_percent":5,"image""#;
    assert!(text.contains(pair), "{text}");
    let older = text.replace(r#""line_percent":85,"margin_percent":5,"#, "");
    let older = Snapshot::from_bytes(older.as_bytes())
        .unwrap()
        .compaction
        .unwrap();
    assert_eq!((older.line_percent, older.margin_percent), (85, 5));
    let numbers = snapshot.compaction.as_mut().unwrap();
    (numbers.line_percent, numbers.margin_percent) = (90, 3);
    let compaction = snapshot.policy().unwrap().compaction.unwrap();
    assert_eq!(
        (compaction.line_percent, compaction.margin_percent),
        (90, 3)
    );
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

/// 检查点里还在跑的任务那一段 7-8 加过、2026-10-01 实测后去掉（施工 7-8 补）：出厂的快照不再带那两份模板。7-8 以后造的
/// 快照带着，照样读得回来：那两格不认识、不理（`policy.md`「字节和哈希」第 4 条），读成和出厂的一样，造出来的策略也就
/// 不写那一段。
#[test]
fn snapshots_made_since_7_8_still_read_and_their_running_notes_go_unused() {
    let snapshot = engineer();
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    assert!(!text.contains("notes_job"), "出厂的快照不带那两份模板");
    let close = r#""restored_close":"\n</file>\n""#;
    let fields = r#","notes_jobs":"Jobs still running at this checkpoint:\n","notes_job":"- {job} {what} \"{title}\"\n""#;
    let made = text.replacen(close, &format!("{close}{fields}"), 1);
    assert_ne!(made, text, "照 7-8 的样子插进去了");
    let read = Snapshot::from_bytes(made.as_bytes()).unwrap();
    assert_eq!(read, snapshot, "那两格不理，读成和出厂的一样");
    assert!(read.policy().unwrap().notes.is_some());
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

/// 截短重试（施工 6-6 中）：出厂的快照带着字和数（再试 3 次、20%），内核拿到截短的数、摘要没看到的那一段的模板，组装器
/// 拿到补的那一条；以前造的快照里没有，读成没有，不截短，读进来再写出去一字不差；只有数、没有字的也不截短。
#[test]
fn shorten_texts_and_numbers_go_in_and_older_snapshots_lack_them() {
    let snapshot = engineer();
    let policy = snapshot.policy().unwrap();
    let shorten = policy.compaction.unwrap().shorten.unwrap();
    assert_eq!((shorten.tries, shorten.percent), (3, 20));
    assert!(policy.notes.unwrap().uncovered.is_some());
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let start = text.find(r#","shorten":{"truncated""#).unwrap();
    let end = start + text[start..].find(r#""}"#).unwrap() + 2;
    let only_numbers = text[..start].to_string() + &text[end..];
    let read = Snapshot::from_bytes(only_numbers.as_bytes()).unwrap();
    let policy = read.policy().unwrap();
    assert!(policy.compaction.unwrap().shorten.is_none());
    assert!(policy.notes.unwrap().uncovered.is_none());
    let older = only_numbers.replace(r#","shorten":{"tries":3,"percent":20}"#, "");
    assert_ne!(older, only_numbers);
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert!(read.policy().unwrap().compaction.unwrap().shorten.is_none());
    assert_eq!(read.to_bytes(), older.as_bytes());
}

/// 隔离式那一句 system（施工 6-6 下）：出厂的快照带着，内核改走隔离式、组装器拿到那一句；以前造的快照里没有，读成没有，
/// 不改走，读进来再写出去一字不差。
#[test]
fn the_isolated_system_line_goes_in_and_older_snapshots_lack_it() {
    let snapshot = engineer();
    assert!(snapshot.policy().unwrap().compaction.unwrap().isolate);
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let start = text.find(r#","summarize_system":""#).unwrap();
    let rest = &text[start + r#","summarize_system":""#.len()..];
    let end = start + r#","summarize_system":""#.len() + rest.find('"').unwrap() + 1;
    let older = text[..start].to_string() + &text[end..];
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert!(!read.policy().unwrap().compaction.unwrap().isolate);
    assert_eq!(read.to_bytes(), older.as_bytes());
}

/// 摘要指令拆成三份（施工 6-8）：出厂的快照带着正文、要求前面那一行、最后那一句；以前造的快照只有整份的正文，另两份读成
/// 空的，读进来再写出去一字不差，拼出来的摘要请求和出厂的一字不差。
#[test]
fn the_summary_instruction_is_split_and_older_snapshots_read_whole() {
    let snapshot = engineer();
    let texts = snapshot.core.compaction.as_ref().unwrap();
    assert_eq!(texts.summarize_instructions, "\nAdditional Instructions:\n");
    assert!(
        texts
            .summarize_end
            .starts_with("\nReply with the <analysis> block")
    );
    let json = |text: &str| serde_json::to_string(text).unwrap();
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let split = format!(
        r#""summarize_task":{},"summarize_instructions":{},"summarize_end":{}"#,
        json(&texts.summarize_task),
        json(&texts.summarize_instructions),
        json(&texts.summarize_end)
    );
    assert!(text.contains(&split), "{text}");
    let whole = format!("{}{}", texts.summarize_task, texts.summarize_end);
    let older = text.replace(&split, &format!(r#""summarize_task":{}"#, json(&whole)));
    assert_ne!(older, text);
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    let old_texts = read.core.compaction.as_ref().unwrap();
    assert_eq!(old_texts.summarize_instructions, "");
    assert_eq!(old_texts.summarize_end, "");
    assert_eq!(read.to_bytes(), older.as_bytes(), "读进来再写出去一字不差");
    let history = miyu_kernel::history::History::default();
    let upto = miyu_kernel::id::Seq::new(1).unwrap();
    let summarize = |snapshot: &Snapshot| {
        snapshot
            .policy()
            .unwrap()
            .assembler
            .summarize(&history, upto, None, None)
            .messages
    };
    assert_eq!(
        summarize(&read),
        summarize(&snapshot),
        "没附要求的，拼出来一字不差"
    );
}

/// 两种回报的写法（施工 7-2）：出厂的快照带着，组装器照它把回报渲染成带标签的事实；写坏了的造不出策略，说是哪一类；以前造
/// 的快照里没有，读成没有、不渲染，读进来再写出去一字不差。
#[test]
fn job_report_texts_go_in_and_older_snapshots_lack_them() {
    use miyu_kernel::event::Event;
    use miyu_kernel::history::History;
    let lines = [
        r#"{"seq":1,"at":"2026-09-25T07:00:00.000Z","kind":"session.created","by":{"kind":"kernel"},"body":{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}}"#,
        r#"{"seq":2,"at":"2026-09-25T07:00:00.000Z","kind":"message.user","by":{"kind":"person","account":"alice"},"body":{"blocks":[]}}"#,
        r#"{"seq":3,"at":"2026-09-25T07:00:00.000Z","kind":"turn.started","turn":3,"by":{"kind":"kernel"},"body":{"trigger":2}}"#,
        r#"{"seq":4,"at":"2026-09-25T07:00:00.000Z","kind":"message.assistant","turn":3,"by":{"kind":"kernel"},"body":{"blocks":[{"type":"tool_call","call_id":"call_4_1","name":"shell","args":"{}"}],"seen":3}}"#,
        r#"{"seq":5,"at":"2026-09-25T07:00:00.000Z","kind":"tool.result","turn":3,"by":{"kind":"kernel"},"body":{"call_id":"call_4_1","status":"ok","blocks":[],"effects":[{"kind":"job.started","job":"j1","what":"command","title":"跑测试"}]}}"#,
        r#"{"seq":6,"at":"2026-09-25T07:00:00.000Z","kind":"job.reported","by":{"kind":"kernel"},"body":{"job":"j1","reason":"aborted"}}"#,
    ];
    let mut history = History::default();
    for line in lines {
        history.append(Event::from_line(line).unwrap());
    }
    let rendered = |snapshot: &Snapshot| {
        let request = snapshot.policy().unwrap().assembler.assemble(&history);
        String::from_utf8(request.canonical_bytes()).unwrap()
    };
    let snapshot = engineer();
    assert!(
        rendered(&snapshot)
            .contains(r#"<command-ended job=\"j1\" title=\"跑测试\" reason=\"aborted\">"#)
    );
    let mut broken = snapshot.clone();
    if let Some(jobs) = broken.core.jobs.as_mut() {
        jobs.command_exit = "Exit code {signal}.".to_string();
    }
    let error = broken.policy().err().unwrap();
    assert!(
        matches!(
            error,
            BuildError::Texts {
                which: "job report texts",
                ..
            }
        ),
        "{error:?}"
    );
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let start = text.find(r#","jobs":{"#).unwrap();
    let end = start + text[start..].find(r#""}"#).unwrap() + 2;
    let older = text[..start].to_string() + &text[end..];
    let read = Snapshot::from_bytes(older.as_bytes()).unwrap();
    assert!(read.core.jobs.is_none());
    assert!(!rendered(&read).contains("command-ended"));
    assert_eq!(read.to_bytes(), older.as_bytes());
}

/// 任务用的数（施工 7-6），后面是回顾用的数（施工 3-8 四补）、起标题用的数（施工 3-8 五补），在快照的最后。
const JOB_NUMBERS: &str = r#","jobs":{"report_chars":30000}"#;
const RECAP_NUMBERS: &str = r#","recap":{"turns":8,"tokens":8192}"#;
const TITLE_NUMBERS: &str = r#","title":{"tokens":1024,"chars":50,"tries":2}"#;

/// 后台运行关着的（施工 T-1 上）：写在最后、读得回来；开着的不写（上面那份的字节以人格的指纹收尾）。
#[test]
fn a_foreground_session_is_written_last_and_read_back() {
    let mut one = engineer();
    one.foreground = true;
    let text = String::from_utf8(one.to_bytes()).unwrap();
    assert!(text.ends_with(",\"foreground\":true}"), "{text}");
    assert_eq!(Snapshot::from_bytes(&one.to_bytes()), Ok(one));
}

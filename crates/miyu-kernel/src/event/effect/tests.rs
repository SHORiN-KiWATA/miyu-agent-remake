//! 效果的测试（施工 4-6 上）：三种认识的读写一字不差；一行都没显示的不写 `lines`，新建的 `before` 写成
//! `null`；不认识的种类整块原样留着；缺了 `kind`、认识的种类缺了字段，报错。`job.started`（施工 7-1）：两种任务
//! 读写一字不差，不认识的 `what` 原样留着，后台命令不写 `session`。`job.messaged`（施工 7-7）读写一字不差。

use super::*;

/// 一个字母 `a` 的内容哈希。
const A: &str = "sha256:ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb";
/// 空内容的哈希。
const EMPTY: &str = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn hash(text: &str) -> ContentHash {
    serde_json::from_str(&format!("\"{text}\"")).unwrap()
}

/// 读进来，再写出去：一字不差。交回读到的。
fn round_trip(json: &str) -> Effect {
    let effect: Effect = serde_json::from_str(json).unwrap();
    assert_eq!(serde_json::to_string(&effect).unwrap(), json);
    effect
}

#[test]
fn the_three_kinds_round_trip() {
    let read = format!(
        r#"{{"kind":"file.read","path":"/home/me/src/lib.rs","lines":[1,37],"hash":"{A}"}}"#
    );
    assert_eq!(
        round_trip(&read),
        Effect::FileRead(FileRead {
            path: "/home/me/src/lib.rs".to_string(),
            lines: Some([1, 37]),
            hash: hash(A),
        })
    );
    let changed = format!(
        r#"{{"kind":"file.changed","path":"/home/me/src/lib.rs","before":"{A}","after":"{EMPTY}"}}"#
    );
    assert_eq!(
        round_trip(&changed),
        Effect::FileChanged(FileChanged {
            path: "/home/me/src/lib.rs".to_string(),
            before: Some(hash(A)),
            after: hash(EMPTY),
        })
    );
    let trashed = r#"{"kind":"file.trashed","path":"/home/me/notes.md","trash":"/home/me/.local/share/Trash/files/notes.md"}"#;
    assert_eq!(
        round_trip(trashed),
        Effect::FileTrashed(FileTrashed {
            path: "/home/me/notes.md".to_string(),
            trash: "/home/me/.local/share/Trash/files/notes.md".to_string(),
        })
    );
}

#[test]
fn nothing_shown_has_no_lines_and_a_new_file_has_a_null_before() {
    let empty = Effect::FileRead(FileRead {
        path: "/e".to_string(),
        lines: None,
        hash: hash(EMPTY),
    });
    assert_eq!(
        serde_json::to_string(&empty).unwrap(),
        format!(r#"{{"kind":"file.read","path":"/e","hash":"{EMPTY}"}}"#)
    );
    let created = Effect::FileChanged(FileChanged {
        path: "/n".to_string(),
        before: None,
        after: hash(A),
    });
    assert_eq!(
        serde_json::to_string(&created).unwrap(),
        format!(r#"{{"kind":"file.changed","path":"/n","before":null,"after":"{A}"}}"#)
    );
    // 没写 `before` 的，当新建读。
    let without: Effect = serde_json::from_str(&format!(
        r#"{{"kind":"file.changed","path":"/n","after":"{A}"}}"#
    ))
    .unwrap();
    assert_eq!(without, created);
}

/// 图纸上的两条（`agents.md`「对外的样子」）：一个子代理带着会话，一个后台命令没有。
#[test]
fn a_started_job_round_trips() {
    let agent = r#"{"kind":"job.started","job":"j2","what":"agent","title":"查 CI 为什么红","session":"01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91"}"#;
    assert_eq!(
        round_trip(agent),
        Effect::JobStarted(JobStarted {
            job: JobId::parse("j2").unwrap(),
            what: JobKind::Agent,
            title: "查 CI 为什么红".to_string(),
            session: Some(SessionId::parse("01a0d78c-ca52-7d19-8b64-0e3f5a7c2d91").unwrap()),
        })
    );
    let command = Effect::JobStarted(JobStarted {
        job: JobId::parse("j1").unwrap(),
        what: JobKind::Command,
        title: "跑全部测试".to_string(),
        session: None,
    });
    let json = r#"{"kind":"job.started","job":"j1","what":"command","title":"跑全部测试"}"#;
    assert_eq!(
        serde_json::to_string(&command).unwrap(),
        json,
        "没有会话的不写这一格"
    );
    assert_eq!(round_trip(json), command);
    // 写成 null 的当没有。
    let null: Effect = serde_json::from_str(&json.replace("}", r#","session":null}"#)).unwrap();
    assert_eq!(null, command);
}

/// 新版本才有的任务种类：原样留着，写出去还是原样；会话照写着的读。
#[test]
fn an_unknown_job_kind_is_kept_as_it_is() {
    let json = r#"{"kind":"job.started","job":"j3","what":"cron","title":"每天八点"}"#;
    match round_trip(json) {
        Effect::JobStarted(started) => assert_eq!(started.what, JobKind::Other("cron".to_string())),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_unknown_kind_is_kept_as_it_is() {
    let json = r#"{"kind":"diagram.drawn", "file" : "a.svg","scale":1.50}"#;
    let effect = round_trip(json);
    assert!(matches!(effect, Effect::Unknown(_)), "{effect:?}");
}

/// 给子代理留了言（施工 7-7）：只带编号，读写一字不差。
#[test]
fn a_message_to_a_subagent_round_trips() {
    assert_eq!(
        round_trip(r#"{"kind":"job.messaged","job":"j2"}"#),
        Effect::JobMessaged(JobMessaged {
            job: JobId::new(2).unwrap()
        })
    );
}

#[test]
fn broken_effects_are_errors() {
    for json in [
        r#"{"path":"/a"}"#,
        r#"{"kind":"file.read","path":"/a"}"#,
        r#"{"kind":"file.changed","path":"/a","before":null}"#,
        r#"{"kind":"file.read","path":"/a","hash":"md5:00"}"#,
        r#"{"kind":"file.trashed","path":"/a"}"#,
        r#"{"kind":"job.started","what":"command","title":"t"}"#,
        r#"{"kind":"job.started","job":"j0","what":"command","title":"t"}"#,
        r#"{"kind":"job.started","job":"1","what":"command","title":"t"}"#,
        r#"{"kind":"job.started","job":"j1","title":"t"}"#,
        r#"{"kind":"job.started","job":"j1","what":"command"}"#,
        r#"{"kind":"job.started","job":"j1","what":"agent","title":"t","session":"s-1"}"#,
        r#"{"kind":"job.messaged"}"#,
        r#"{"kind":"job.messaged","job":"j0"}"#,
    ] {
        assert!(serde_json::from_str::<Effect>(json).is_err(), "{json}");
    }
}

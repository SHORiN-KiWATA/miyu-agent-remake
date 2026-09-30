//! 事实的测试：模板造的时候就查；三块的写法；一个边界上查哪几块、先后；以前的快照没有会话编号的模板；该不该注入：
//! 第一次、一样、变了、隔着边界变回去、别的来源和别的类不算、压缩以后、撤销以后。日志都先交给账本查过（[`Log`]）。
//! 权限那一块比级别、切了用哪份模板在 `tests/permission.rs`（施工 2-7 补）。

mod permission;

use super::*;
use crate::event::Event;
use crate::ledger::Ledger;
use crate::template::escape;

const KERNEL: &str = r#"{"kind":"kernel"}"#;
const ALICE: &str = r#"{"kind":"person","account":"alice"}"#;
const MEMORY: &str = r#"{"kind":"module","id":"memory"}"#;
const CREATED: &str = r#"{"owner":"alice","venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#;

/// 替身的模板：短，一眼认得出是哪个字段。测的是换法，和出厂的措辞无关。
fn templates() -> FactTemplates {
    FactTemplates::new(
        r#"<e t="{time}" z="{timezone}" d="{cwd}"/>"#,
        r#"<p l="{level}"/>"#,
        "<cut/>",
        Some(r#"<s i="{id}"/>"#),
        None,
    )
    .unwrap()
}

/// 会话编号：一个真会话的写法。
const SESSION: &str = "01a0f233-cfec-7023-8ed5-2a037a1d5ec8";

fn session() -> SessionId {
    SessionId::parse(SESSION).unwrap()
}

/// 边界上的此刻。
fn now() -> Timestamp {
    Timestamp::parse("2026-09-25T07:04:05.140Z").unwrap()
}

fn environment(cwd: &str) -> Environment {
    Environment {
        offset: UtcOffset::from_minutes(540).unwrap(),
        cwd: cwd.to_string(),
        dirs: Vec::new(),
    }
}

fn permission(level: Level, read_only: bool) -> Permission {
    Permission { level, read_only }
}

/// 一块事实。
fn fact(kind: &str, text: &str) -> ContextInjected {
    ContextInjected {
        kind: FactKind::parse(kind).unwrap(),
        text: text.to_string(),
    }
}

/// 一段日志：照先后追加，序号自动往后编。每一条都先交给账本查过，再交给有效历史。
struct Log {
    ledger: Ledger,
    history: History,
    /// 正在进行的回合。
    turn: Option<u64>,
}

impl Log {
    /// 一个刚创建的会话。
    fn new() -> Log {
        let mut log = Log {
            ledger: Ledger::default(),
            history: History::default(),
            turn: None,
        };
        log.push(KERNEL, "session.created", CREATED);
        log
    }

    /// 追加一条，回合里的带上回合。返回它的序号。
    fn push(&mut self, by: &str, kind: &str, body: &str) -> u64 {
        let seq = self.ledger.next_seq().get();
        let turn = self
            .turn
            .map(|turn| format!(r#""turn":{turn},"#))
            .unwrap_or_default();
        let line = format!(
            r#"{{"seq":{seq},"at":"2026-09-25T07:00:00.000Z","kind":"{kind}",{turn}"by":{by},"body":{body}}}"#
        );
        let event = Event::from_line(&line).unwrap();
        self.ledger.append(&event).unwrap();
        self.history.append(event);
        seq
    }

    /// alice 说一句，开一个回合。返回回合的编号。
    fn start(&mut self) -> u64 {
        let message = self.push(ALICE, "message.user", r#"{"blocks":[]}"#);
        self.turn = Some(self.ledger.next_seq().get());
        self.push(
            KERNEL,
            "turn.started",
            &format!(r#"{{"trigger":{message}}}"#),
        )
    }

    /// 结束回合。
    fn end(&mut self) {
        self.push(KERNEL, "turn.ended", r#"{"reason":"completed"}"#);
        self.turn = None;
    }

    /// `by` 注入一块事实。
    fn inject(&mut self, by: &str, kind: &str, text: &str) {
        let body = format!(
            r#"{{"kind":"{kind}","text":{}}}"#,
            serde_json::to_string(text).unwrap()
        );
        self.push(by, "context.injected", &body);
    }

    /// 压缩，替代到上一条为止：在正在进行的回合里（压缩带着它所在的回合，施工 6-9）。
    fn compact(&mut self) {
        let upto = self.ledger.next_seq().get() - 1;
        self.push(
            KERNEL,
            "context.compacted",
            &format!(r#"{{"upto":{upto},"summary":"…"}}"#),
        );
    }

    /// 撤销一个回合。
    fn revert(&mut self, turn: u64) {
        self.push(ALICE, "turn.reverted", &format!(r#"{{"turns":[{turn}]}}"#));
    }

    /// 内核这一次该注入这几块里的哪几块，只看原文。
    fn changed(&self, facts: &[ContextInjected]) -> Vec<String> {
        changed(&self.history, &By::Kernel, facts.to_vec())
            .into_iter()
            .map(|fact| fact.text)
            .collect()
    }
}

#[test]
fn a_template_asking_for_a_field_it_does_not_have_is_refused() {
    let env = FactTemplates::new(
        r#"<e w="{weather}"/>"#,
        r#"<p l="{level}"/>"#,
        "<cut/>",
        None,
        None,
    )
    .unwrap_err();
    assert!(env.why.contains("weather"), "{env}");
    let permission = FactTemplates::new(
        r#"<e t="{time}"/>"#,
        r#"<p t="{time}"/>"#,
        "<cut/>",
        None,
        None,
    )
    .unwrap_err();
    assert!(permission.why.contains("time"), "{permission}");
    // 被打断的那一句没有字段。
    let cut = FactTemplates::new(
        r#"<e t="{time}"/>"#,
        r#"<p l="{level}"/>"#,
        r#"<cut n="{count}"/>"#,
        None,
        None,
    )
    .unwrap_err();
    assert!(cut.why.contains("count"), "{cut}");
    // 会话编号那一块只有 `id`（施工 1-13 再补）。
    let session = FactTemplates::new(
        r#"<e t="{time}"/>"#,
        r#"<p l="{level}"/>"#,
        "<cut/>",
        Some(r#"<s t="{time}"/>"#),
        None,
    )
    .unwrap_err();
    assert!(session.why.contains("time"), "{session}");
}

#[test]
fn a_broken_template_is_refused() {
    assert!(
        FactTemplates::new(
            r#"<e t="{time"/>"#,
            r#"<p l="{level}"/>"#,
            "<cut/>",
            None,
            None
        )
        .is_err()
    );
    assert!(
        FactTemplates::new(
            r#"<e t="{time}"/>"#,
            r#"<p l="{level}"/>"#,
            "<cut/>",
            Some(r#"<s i="{id"/>"#),
            None,
        )
        .is_err()
    );
}

#[test]
fn the_session_block_has_the_session_id() {
    let block = templates().session(&session()).unwrap();
    assert_eq!(block.kind.as_str(), "session");
    assert_eq!(block.text, format!(r#"<s i="{SESSION}"/>"#));
}

#[test]
fn a_boundary_checks_the_env_the_permission_and_the_session_in_that_order() {
    let facts = templates().boundary(
        &History::default(),
        now(),
        &environment("~/src/miyu"),
        &permission(Level::Workspace, false),
        &session(),
    );
    let kinds: Vec<&str> = facts.iter().map(|fact| fact.kind.as_str()).collect();
    assert_eq!(kinds, ["env", "permission", "session"]);
    assert_eq!(facts[2].text, format!(r#"<s i="{SESSION}"/>"#));
}

#[test]
fn older_templates_without_the_session_one_have_no_session_block() {
    let older = FactTemplates::new(
        r#"<e t="{time}" z="{timezone}" d="{cwd}"/>"#,
        r#"<p l="{level}"/>"#,
        "<cut/>",
        None,
        None,
    )
    .unwrap();
    assert_eq!(older.session(&session()), None);
    let facts = older.boundary(
        &History::default(),
        now(),
        &environment("~/src/miyu"),
        &permission(Level::Workspace, false),
        &session(),
    );
    let kinds: Vec<&str> = facts.iter().map(|fact| fact.kind.as_str()).collect();
    assert_eq!(kinds, ["env", "permission"]);
}

#[test]
fn the_env_block_has_the_hour_the_timezone_and_the_directory() {
    let block = templates().env(now(), &environment("~/src/miyu"));
    assert_eq!(block.kind.as_str(), "env");
    assert_eq!(
        block.text,
        r#"<e t="Fri 2026-09-25 16:00–17:00" z="UTC+09:00" d="~/src/miyu"/>"#
    );
}

#[test]
fn the_directory_is_escaped() {
    let cwd = "~/a\"b<c>";
    let block = templates().env(now(), &environment(cwd));
    assert_eq!(
        block.text,
        format!(
            r#"<e t="Fri 2026-09-25 16:00–17:00" z="UTC+09:00" d="{}"/>"#,
            escape(cwd)
        )
    );
    assert!(!block.text.contains("a\"b"));
}

#[test]
fn the_permission_block_names_the_level_in_effect() {
    for (permission, level) in [
        (permission(Level::Workspace, false), "workspace"),
        (permission(Level::Full, false), "full"),
        (permission(Level::Workspace, true), "read_only"),
        (permission(Level::Full, true), "read_only"),
        (
            permission(Level::Other("sudo".to_string()), false),
            "read_only",
        ),
    ] {
        let block = templates().permission(&permission);
        assert_eq!(block.kind.as_str(), "permission");
        assert_eq!(block.text, format!(r#"<p l="{level}"/>"#), "{permission:?}");
    }
}

#[test]
fn the_first_time_every_block_is_injected() {
    let mut log = Log::new();
    log.start();
    assert_eq!(
        log.changed(&[fact("env", "A"), fact("permission", "W")]),
        ["A", "W"]
    );
}

#[test]
fn a_block_she_saw_last_is_not_injected_again() {
    let mut log = Log::new();
    log.start();
    log.inject(KERNEL, "env", "A");
    log.inject(KERNEL, "permission", "W");
    log.end();
    log.start();
    assert!(
        log.changed(&[fact("env", "A"), fact("permission", "W")])
            .is_empty()
    );
}

#[test]
fn a_changed_block_is_injected_and_the_rest_are_not() {
    let mut log = Log::new();
    log.start();
    log.inject(KERNEL, "env", "A");
    log.inject(KERNEL, "permission", "W");
    log.end();
    log.start();
    assert_eq!(
        log.changed(&[fact("env", "B"), fact("permission", "W")]),
        ["B"]
    );
}

#[test]
fn going_back_across_a_boundary_is_injected_again() {
    let mut log = Log::new();
    log.start();
    log.inject(KERNEL, "env", "A");
    log.end();
    log.start();
    log.inject(KERNEL, "env", "B");
    log.end();
    log.start();
    assert_eq!(log.changed(&[fact("env", "A")]), ["A"]);
}

#[test]
fn a_block_from_another_injector_does_not_count() {
    let mut log = Log::new();
    log.start();
    log.inject(MEMORY, "env", "A");
    assert_eq!(log.changed(&[fact("env", "A")]), ["A"]);
}

#[test]
fn a_block_of_another_kind_does_not_count() {
    let mut log = Log::new();
    log.start();
    log.inject(KERNEL, "permission", "A");
    assert_eq!(log.changed(&[fact("env", "A")]), ["A"]);
}

#[test]
fn after_a_compaction_every_block_is_injected_again() {
    let mut log = Log::new();
    log.start();
    log.inject(KERNEL, "env", "A");
    log.inject(KERNEL, "permission", "W");
    log.end();
    log.start();
    log.compact();
    assert_eq!(
        log.changed(&[fact("env", "A"), fact("permission", "W")]),
        ["A", "W"]
    );
}

#[test]
fn after_an_undo_the_block_before_it_counts() {
    let mut log = Log::new();
    log.start();
    log.inject(KERNEL, "env", "A");
    log.end();
    let second = log.start();
    log.inject(KERNEL, "env", "B");
    log.end();
    log.revert(second);
    log.start();
    assert!(log.changed(&[fact("env", "A")]).is_empty());
    assert_eq!(log.changed(&[fact("env", "B")]), ["B"]);
}

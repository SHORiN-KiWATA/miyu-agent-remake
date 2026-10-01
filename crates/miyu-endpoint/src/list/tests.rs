//! 照整份日志算的几样（施工 C-3）：工作目录照最后一条带 `cwd` 的，不带的不盖；最近一次动静是最后一条的时刻；标题、置顶照旧。
//! 读索引的和整份读的一字不差（施工 3-8 七补，`indexed.rs`）。

mod indexed;
mod speed;

use miyu_kernel::event::{Body, Event};

use super::*;

fn event(line: &str) -> Event {
    Event::from_line(line).expect("合写法")
}

pub(super) const CREATED: &str = r#"{"seq":1,"at":"2026-09-25T07:00:00.000Z","kind":"session.created","by":{"kind":"person","account":"alice"},"cause":"c1","body":{"owner":"alice","venue":"local","policy":"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894","permission":{"level":"workspace","read_only":false},"cwd":"~/a"}}"#;

fn read(lines: &[&str]) -> Row {
    let created = event(CREATED);
    assert!(matches!(created.body, Body::SessionCreated(_)));
    let mut read =
        Row::new(SessionId::parse(A).expect("合写法"), &created).expect("第一条是 session.created");
    for line in lines {
        read.see(&event(line));
    }
    read
}

#[test]
fn the_working_directory_is_the_last_one_written_down() {
    assert_eq!(read(&[]).cwd.as_deref(), Some("~/a"), "只有造会话时的");
    let moved = r#"{"seq":2,"at":"2026-09-25T07:01:00.000Z","kind":"turn.started","turn":2,"by":{"kind":"kernel"},"cause":"c2","body":{"cwd":"~/b"}}"#;
    let silent = r#"{"seq":3,"at":"2026-09-25T07:02:00.000Z","kind":"turn.started","turn":3,"by":{"kind":"kernel"},"cause":"c3","body":{}}"#;
    assert_eq!(read(&[CREATED, moved]).cwd.as_deref(), Some("~/b"));
    assert_eq!(
        read(&[CREATED, moved, silent]).cwd.as_deref(),
        Some("~/b"),
        "不带 cwd 的一轮不盖"
    );
}

#[test]
fn last_active_is_the_time_of_the_last_event_of_any_kind() {
    assert_eq!(
        read(&[]).last_active.to_string(),
        "2026-09-25T07:00:00.000Z"
    );
    let renamed = r#"{"seq":2,"at":"2026-09-25T09:30:00.000Z","kind":"session.meta_changed","by":{"kind":"person","account":"alice"},"cause":"c2","body":{"title":"发版"}}"#;
    let got = read(&[CREATED, renamed]);
    assert_eq!(got.last_active.to_string(), "2026-09-25T09:30:00.000Z");
    assert_eq!(got.title, "发版");
}

/// 一个临时的数据根，里面照 `logs` 写好几个会话的第一段日志：编号、一行行事件。
pub(super) struct Site(pub(super) std::path::PathBuf, pub(super) DataRoot);

impl Site {
    pub(super) fn new(logs: &[(&str, &[&str])]) -> Site {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("miyu-endpoint-list-{}-{n}", std::process::id()));
        let root = DataRoot::locate(&miyu_store::env::Env {
            platform: miyu_store::env::Platform::current(),
            miyu_home: Some(dir.clone().into_os_string()),
            home: None,
            xdg_cache_home: None,
            local_app_data: None,
            miyu_resources: None,
            exe: None,
        })
        .expect("MIYU_HOME 是绝对路径");
        root.prepare().expect("建得了骨架");
        for (id, lines) in logs {
            let session = root.session_dir(&alice(), &SessionId::parse(id).expect("合写法"));
            std::fs::create_dir_all(&session).expect("建得了");
            let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
            std::fs::write(session.join("000000000001.jsonl"), text).expect("写得进");
        }
        Site(dir, root)
    }
}

impl Drop for Site {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn alice() -> AccountId {
    AccountId::parse("alice").expect("合写法")
}

/// 很早以前的日志：`session.created` 不带 `cwd`，也没开过带 `cwd` 的回合。
pub(super) const OLD: &str = r#"{"seq":1,"at":"2026-09-25T07:00:00.000Z","kind":"session.created","by":{"kind":"person","account":"alice"},"cause":"c1","body":{"owner":"alice","venue":"local","policy":"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894","permission":{"level":"workspace","read_only":false}}}"#;

pub(super) const A: &str = "0192f3a0-1111-7abc-8def-001122334455";
pub(super) const B: &str = "0192f3a0-2222-7abc-8def-5566778899aa";

#[test]
fn a_log_with_no_working_directory_says_tilde_and_busy_comes_from_the_table() {
    let site = Site::new(&[(A, &[OLD]), (B, &[CREATED])]);
    let busy = BTreeSet::from([SessionId::parse(B).expect("合写法")]);
    let listed = scan(
        &site.1,
        &alice(),
        None,
        &busy,
        |_| true,
        None,
        &Stop::default(),
    )
    .expect("读得了");
    let got: Vec<(&str, &str, bool)> = listed
        .iter()
        .map(|one| (one.id.as_str(), one.cwd.as_str(), one.busy))
        .collect();
    assert_eq!(
        got,
        [(B, "~/a", true), (A, "~", false)],
        "照编号倒着排；没记过工作目录的和会话表载入时一样当报来的是 ~"
    );
}

#[test]
fn a_raised_flag_stops_before_the_next_session() {
    let site = Site::new(&[(A, &[OLD]), (B, &[CREATED])]);
    let stop = Stop::default();
    stop.raise();
    let listed = scan(
        &site.1,
        &alice(),
        None,
        &BTreeSet::new(),
        |_| true,
        None,
        &stop,
    )
    .expect("读得了");
    assert!(listed.is_empty(), "一个都不读");
}

//! 读别的会话（施工 C-4，`docs/blueprint/tools/history.md`「怎么走」第二条，`cross-session.md` 第二条）：`session` 参数认出来
//! 的是另一个会话，读的是它的日志，不是这次调用自己的；认成她自己的照没写，不开任何别的日志；找不到、对得上不止一个、
//! 这个会话不能读别的会话（没有列会话的端口）各一句拒绝，一条日志都不读；时刻照这个会话自己的时区。

use std::sync::{Arc, Mutex};

use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;
use miyu_tool::{Listing, MainSession, Opening, SessionsPort};

use super::*;

/// 这个会话自己：短编号 `22334455`。
const THIS: &str = "0192f3a0-1111-7abc-8def-001122334455";
/// 另一个会话：短编号 `778899aa`。
const OTHER: &str = "0192f3a0-2222-7abc-8def-5566778899aa";
/// 第三个会话，和 `OTHER` 共享后 8 位，用来测「对得上不止一个」。
const TWIN: &str = "0192f3a0-3333-7abc-8def-eeee778899aa";

fn id(text: &str) -> SessionId {
    SessionId::parse(text).expect("合写法")
}

fn peer(text: &str) -> MainSession {
    MainSession {
        id: id(text),
        title: String::new(),
        cwd: "~".to_string(),
        busy: false,
        last_active: Timestamp::parse("2026-09-29T05:00:00.000Z").expect("合写法"),
    }
}

/// 读了一次就 panic：断言出错、认成她自己的几种走法一条日志都不读（怎么走第二条第 1 款）。
struct NeverRead;

impl ReadLog for NeverRead {
    fn read(&self, _each: &mut dyn FnMut(Vec<Event>) -> bool) -> Result<(), String> {
        panic!("这几种走法不该读任何日志")
    }
}

/// 假的列会话、开日志的端口：`others` 是列得到的别的会话；`logs` 是 `open` 认得出来的日志，认不出的（不在
/// 表里的）交回原因；`list_failure`、`open_failure` 设了就在那一步出错；`opened` 记着被开过的会话，用来断言
/// 认成她自己的那几种走法一次都没开别的日志。
struct Peers {
    this: SessionId,
    others: Vec<SessionId>,
    logs: std::collections::BTreeMap<String, Vec<Vec<Event>>>,
    list_failure: Option<String>,
    open_failure: Option<String>,
    opened: Mutex<Vec<SessionId>>,
}

impl Peers {
    fn new(this: &str, others: &[&str]) -> Peers {
        Peers {
            this: id(this),
            others: others.iter().map(|other| id(other)).collect(),
            logs: std::collections::BTreeMap::new(),
            list_failure: None,
            open_failure: None,
            opened: Mutex::new(Vec::new()),
        }
    }

    /// 让 `session` 开得到 `segments`。
    fn with_log(mut self, session: &str, segments: Vec<Vec<Event>>) -> Peers {
        self.logs.insert(session.to_string(), segments);
        self
    }

    /// 列会话那一步出错。
    fn fails_listing(mut self, error: &str) -> Peers {
        self.list_failure = Some(error.to_string());
        self
    }

    /// 开日志那一步出错。
    fn fails_opening(mut self, error: &str) -> Peers {
        self.open_failure = Some(error.to_string());
        self
    }

    /// 被开过的会话，照先后。
    fn opened(&self) -> Vec<SessionId> {
        self.opened
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl SessionsPort for Peers {
    fn this(&self) -> &SessionId {
        &self.this
    }

    fn list<'a>(&'a self, _stop: &'a Stop) -> Listing<'a> {
        let result = match &self.list_failure {
            Some(error) => Err(error.clone()),
            None => Ok(self.others.iter().map(|id| peer(id.as_str())).collect()),
        };
        Box::pin(async move { result })
    }

    fn open<'a>(&'a self, session: &'a SessionId) -> Opening<'a> {
        self.opened
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(session.clone());
        let result = match &self.open_failure {
            Some(error) => Err(error.clone()),
            None => self
                .logs
                .get(session.as_str())
                .map(|segments| {
                    Log::new(Segments {
                        segments: segments.clone(),
                        broken: None,
                    })
                })
                .ok_or_else(|| format!("no log recorded for {session}")),
        };
        Box::pin(async move { result })
    }
}

/// 一段日志，只有一条消息，方便认出读的是哪一份。
fn log_with(seq: u64, text: &str) -> Vec<Vec<Event>> {
    vec![vec![event(
        seq,
        "2026-09-29T05:00:00.000Z",
        "message.user",
        None,
        alice(),
        json!({"blocks": [{"type": "text", "text": text}]}),
    )]]
}

/// 在这个会话自己的日志 `own`、列会话端口 `sessions` 上跑一次，时区 UTC。
async fn run_peers(args: Value, own: Vec<Vec<Event>>, sessions: &Arc<Peers>) -> Done {
    let call = Call {
        sessions: Some(Arc::clone(sessions) as Arc<dyn SessionsPort>),
        ..call(args, Some(log(own)), 0)
    };
    run(call).await
}

#[tokio::test]
async fn a_session_id_reads_that_sessions_log_not_this_calls() {
    let sessions = Arc::new(Peers::new(THIS, &[OTHER]).with_log(OTHER, log_with(9, "来自 OTHER")));
    let done = run_peers(
        json!({"session": "778899aa"}),
        log_with(2, "我自己的"),
        &sessions,
    )
    .await;
    assert!(!done.error);
    assert_eq!(text(&done), "#9 2026-09-29 05:00 user\n来自 OTHER\n");
    assert_eq!(sessions.opened(), vec![id(OTHER)], "只开了认出来的那一个");
}

#[tokio::test]
async fn resolving_to_herself_opens_nothing_and_reads_her_own_log() {
    let sessions = Arc::new(Peers::new(THIS, &[OTHER]).with_log(OTHER, log_with(9, "来自 OTHER")));
    let done = run_peers(
        json!({"session": "22334455"}),
        log_with(2, "我自己的"),
        &sessions,
    )
    .await;
    assert!(!done.error);
    assert_eq!(text(&done), "#2 2026-09-29 05:00 user\n我自己的\n");
    assert!(sessions.opened().is_empty(), "认成自己的不该开别的日志");
}

#[tokio::test]
async fn an_unknown_session_id_is_rejected_without_reading_a_log() {
    let sessions = Arc::new(Peers::new(THIS, &[OTHER]));
    let call = Call {
        sessions: Some(Arc::clone(&sessions) as Arc<dyn SessionsPort>),
        ..call(json!({"session": "00000000"}), Some(Log::new(NeverRead)), 0)
    };
    let done = run(call).await;
    assert!(done.error);
    assert_eq!(text(&done), "No session has the id \"00000000\".\n");
    assert!(
        human(&done).contains("history/no-session"),
        "{}",
        human(&done)
    );
    assert!(sessions.opened().is_empty());
}

#[tokio::test]
async fn an_id_shared_by_two_sessions_is_ambiguous_without_reading_a_log() {
    let sessions = Arc::new(Peers::new(THIS, &[OTHER, TWIN]));
    let call = Call {
        sessions: Some(Arc::clone(&sessions) as Arc<dyn SessionsPort>),
        ..call(json!({"session": "778899aa"}), Some(Log::new(NeverRead)), 0)
    };
    let done = run(call).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "\"778899aa\" matches more than one session. Use the full id.\n"
    );
    assert!(sessions.opened().is_empty());
}

#[tokio::test]
async fn without_a_sessions_port_the_parameter_is_refused_without_reading_a_log() {
    let call = Call {
        sessions: None,
        ..call(json!({"session": "778899aa"}), Some(Log::new(NeverRead)), 0)
    };
    let done = run(call).await;
    assert!(done.error);
    assert_eq!(text(&done), "This session cannot read other sessions.\n");
    assert!(
        human(&done).contains("history/not-here"),
        "{}",
        human(&done)
    );
}

#[tokio::test]
async fn a_failure_listing_or_opening_says_the_log_could_not_be_read() {
    let sessions = Arc::new(Peers::new(THIS, &[OTHER]).fails_listing("the core is shutting down"));
    let done = run_peers(json!({"session": "778899aa"}), Vec::new(), &sessions).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "Could not read the log: the core is shutting down\n"
    );

    let sessions = Arc::new(Peers::new(THIS, &[OTHER]).fails_opening("session directory is gone"));
    let done = run_peers(json!({"session": "778899aa"}), Vec::new(), &sessions).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "Could not read the log: session directory is gone\n"
    );
}

#[tokio::test]
async fn an_empty_session_is_the_same_as_not_writing_it() {
    let sessions = Arc::new(Peers::new(THIS, &[OTHER]));
    let done = run_peers(json!({"session": ""}), log_with(2, "我自己的"), &sessions).await;
    assert!(!done.error);
    assert_eq!(text(&done), "#2 2026-09-29 05:00 user\n我自己的\n");
    assert!(sessions.opened().is_empty());
}

#[tokio::test]
async fn reading_someone_elses_log_still_uses_this_sessions_time_zone() {
    let sessions = Arc::new(Peers::new(THIS, &[OTHER]).with_log(OTHER, log_with(9, "来自 OTHER")));
    let call = Call {
        sessions: Some(Arc::clone(&sessions) as Arc<dyn SessionsPort>),
        // -60 分钟（UTC-1）：和别的测试用的 +09:00、UTC 都不一样，确认真是这个会话的时区，不是巧合对上了。
        ..call(json!({"session": "778899aa"}), Some(log(Vec::new())), -60)
    };
    let done = run(call).await;
    assert!(
        text(&done).starts_with("#9 2026-09-29 04:00 user"),
        "{}",
        text(&done)
    );
}

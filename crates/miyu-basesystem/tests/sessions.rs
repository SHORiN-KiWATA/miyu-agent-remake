//! `sessions`（`docs/blueprint/tools/sessions.md`，施工 C-3）：输出逐字节比；第一行是她自己，最近有动静的在前；没标题的；分页、
//! 过了结尾、没有别的；短编号撞了放长；参数不对的不问端口；没有端口、列不出来、叫停。给人看的说法三种语言都换得出字。

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::SessionId;
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_kernel::tool::Access;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;
use miyu_tool::{Done, Listing, MainSession, Opening, SessionsPort, Stop};

use support::{Site, check, human, readable, resources, said, tool};

/// 她自己：短编号 `22334455`。
const THIS: &str = "0192f3a0-1111-7abc-8def-001122334455";

/// 假的列会话端口：交回给的那几个，或者列不出来；数着被问了几次，记下交给它的旗举没举。
struct Port {
    this: SessionId,
    listed: Result<Vec<MainSession>, String>,
    asked: AtomicUsize,
}

impl Port {
    fn new(listed: Vec<MainSession>) -> Port {
        Port {
            this: id(THIS),
            listed: Ok(listed),
            asked: AtomicUsize::new(0),
        }
    }
}

impl SessionsPort for Port {
    fn this(&self) -> &SessionId {
        &self.this
    }

    fn list<'a>(&'a self, _stop: &'a Stop) -> Listing<'a> {
        self.asked.fetch_add(1, Ordering::Relaxed);
        let listed = self.listed.clone();
        Box::pin(async move { listed })
    }

    /// `sessions` 用不到开别的会话的日志（施工 C-4，那是 `history` 的事）。
    fn open<'a>(&'a self, _session: &'a SessionId) -> Opening<'a> {
        Box::pin(async { Err("not reached in this test".to_string()) })
    }
}

fn id(text: &str) -> SessionId {
    SessionId::parse(text).expect("合写法")
}

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("合写法")
}

/// 一个主会话。
fn main(session: &str, title: &str, cwd: &str, busy: bool, last_active: &str) -> MainSession {
    MainSession {
        id: id(session),
        title: title.to_string(),
        cwd: cwd.to_string(),
        busy,
        last_active: at(last_active),
    }
}

/// 第 `n` 个会话：没标题，在 `~/n<n>`，时刻照 `n` 往后排，越大越新。
fn nth(n: u32) -> MainSession {
    main(
        &format!("0192f3a0-2222-7abc-8def-0000000000{n:02}"),
        "",
        &format!("~/n{n}"),
        false,
        &format!("2026-10-01T06:{n:02}:00.000Z"),
    )
}

/// 交回的那一段字。
fn text(done: &Done) -> &str {
    match done.blocks.as_slice() {
        [Block::Text(Text { text })] => text,
        other => panic!("只有一段字：{other:?}"),
    }
}

/// 照 `port` 调一次 `sessions`，时区东八区。
async fn sessions(port: &Arc<Port>, args: serde_json::Value) -> Done {
    let port = Arc::clone(port) as Arc<dyn SessionsPort>;
    let east8 = UtcOffset::from_minutes(480).expect("在范围里");
    Site::new()
        .done_with_sessions("sessions", args, Some(port), east8, Stop::default())
        .await
}

#[test]
fn it_reads_logs_only_and_takes_limit_and_offset() {
    let spec = tool("sessions").spec().clone();
    assert_eq!(spec.name, "sessions");
    assert_eq!(spec.access, Access::Read);
    let parameters: serde_json::Value =
        serde_json::from_str(spec.parameters.get()).expect("参数格式是 JSON");
    assert_eq!(parameters["properties"]["limit"]["type"], "integer");
    assert_eq!(parameters["properties"]["offset"]["type"], "integer");
    assert!(parameters.get("required").is_none(), "两格都可以不写");
}

#[tokio::test]
async fn the_first_line_is_herself_and_the_newest_comes_first() {
    let port = Arc::new(Port::new(vec![
        main(
            "0192f3a0-3333-7abc-8def-9a9b0c5d77aa",
            "",
            "~/notes",
            false,
            "2026-09-30T14:41:00.000Z",
        ),
        main(
            "0192f3a0-2222-7abc-8def-5566e9f03b21",
            "修 CI",
            "~/src/miyu",
            true,
            "2026-10-01T06:03:12.345Z",
        ),
    ]));
    let done = sessions(&port, json!({})).await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "You are session 22334455.\n\
         e9f03b21 \"修 CI\" in ~/src/miyu: busy, last active 2026-10-01 14:03\n\
         0c5d77aa (untitled) in ~/notes: idle, last active 2026-09-30 22:41\n"
    );
    assert_eq!(done.human, Some(said("sessions/listed").with("count", "2")));
}

#[tokio::test]
async fn ties_go_by_id_newest_first_and_fields_are_escaped() {
    let same = "2026-10-01T06:00:00.000Z";
    let port = Arc::new(Port::new(vec![
        main(
            "0192f3a0-2222-7abc-8def-0000000000a1",
            "",
            "~/a",
            false,
            same,
        ),
        main(
            "0192f3a0-2222-7abc-8def-0000000000a2",
            "say \"hi\"\nnow",
            "C:\\work",
            false,
            same,
        ),
    ]));
    let done = sessions(&port, json!({})).await;
    assert_eq!(
        text(&done),
        "You are session 22334455.\n\
         000000a2 \"say \\u0022hi\\u0022\\nnow\" in C:\\\\work: idle, last active 2026-10-01 14:00\n\
         000000a1 (untitled) in ~/a: idle, last active 2026-10-01 14:00\n"
    );
}

#[tokio::test]
async fn pages_go_by_limit_and_offset() {
    let port = Arc::new(Port::new((1..=5).map(nth).collect()));
    let row = |n: u32| {
        format!("000000{n:02} (untitled) in ~/n{n}: idle, last active 2026-10-01 14:{n:02}\n")
    };
    let first = sessions(&port, json!({"limit": 2})).await;
    assert_eq!(
        text(&first),
        format!(
            "You are session 22334455.\n{}{}(Showing 1-2 of 5. Use offset=2 to see more.)\n",
            row(5),
            row(4)
        )
    );
    assert_eq!(
        first.human,
        Some(said("sessions/listed").with("count", "2"))
    );
    let second = sessions(&port, json!({"limit": "2", "offset": 2})).await;
    assert_eq!(
        text(&second),
        format!(
            "You are session 22334455.\n{}{}(Showing 3-4 of 5. Use offset=4 to see more.)\n",
            row(3),
            row(2)
        ),
        "写成字符串的整数也认"
    );
    let last = sessions(&port, json!({"limit": 2, "offset": 4})).await;
    assert_eq!(
        text(&last),
        format!("You are session 22334455.\n{}", row(1)),
        "到了结尾不接那一句"
    );
    let port = Arc::new(Port::new((1..=21).map(nth).collect()));
    let whole = sessions(&port, json!({})).await;
    assert!(
        text(&whole).ends_with("(Showing 1-20 of 21. Use offset=20 to see more.)\n"),
        "不写是 20 个：{}",
        text(&whole)
    );
}

#[tokio::test]
async fn past_the_end_and_nobody_else() {
    let port = Arc::new(Port::new((1..=5).map(nth).collect()));
    let done = sessions(&port, json!({"offset": 5})).await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "You are session 22334455.\n(You have 5 other sessions. Offset 5 is past the end.)\n"
    );
    assert_eq!(
        done.human,
        Some(said("sessions/past-end").with("total", "5"))
    );
    let port = Arc::new(Port::new(Vec::new()));
    let done = sessions(&port, json!({"offset": 3})).await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "You are session 22334455.\nYou have no other sessions.\n",
        "一个都没有的不说过了结尾"
    );
    assert_eq!(done.human, Some(said("sessions/none")));
}

#[tokio::test]
async fn ids_that_clash_in_the_list_are_written_longer() {
    let when = "2026-10-01T06:00:00.000Z";
    // 她自己和第一个后 8 位一样；第一个和第二个后 12 位也一样；第三个谁都不撞。
    let mut port = Port::new(vec![
        main("0192f3a0-2222-7abc-8def-bbbb22334455", "", "~", false, when),
        main("0192f3a0-3333-7abc-8def-bbbb22334455", "", "~", false, when),
        main("0192f3a0-4444-7abc-8def-0000778899aa", "", "~", false, when),
    ]);
    port.this = id("0192f3a0-1111-7abc-8def-aaaa22334455");
    let done = sessions(&Arc::new(port), json!({})).await;
    let ids: Vec<&str> = text(&done)
        .lines()
        .map(|line| line.split(' ').next().unwrap_or_default())
        .collect();
    assert_eq!(
        text(&done).lines().next(),
        Some("You are session aaaa22334455.")
    );
    assert_eq!(
        ids[1..],
        [
            "778899aa",
            "0192f3a0-3333-7abc-8def-bbbb22334455",
            "0192f3a0-2222-7abc-8def-bbbb22334455",
        ]
    );
}

#[tokio::test]
async fn bad_arguments_do_not_ask_the_port() {
    let port = Arc::new(Port::new((1..=2).map(nth).collect()));
    for args in [
        json!({"limit": 0}),
        json!({"limit": -1}),
        json!({"limit": "x"}),
        json!({"limit": 1.5}),
        json!({"offset": -1}),
        json!({"offset": true}),
        json!({"offset": [1]}),
    ] {
        let done = sessions(&port, args.clone()).await;
        assert!(done.error, "{args}");
        assert!(
            text(&done).starts_with("The arguments are not right: "),
            "{args}：{}",
            text(&done)
        );
        assert_eq!(
            done.human.as_ref().map(|said| said.key.as_str()),
            Some("software/basesystem/common/bad-args")
        );
    }
    assert_eq!(port.asked.load(Ordering::Relaxed), 0, "端口一次都没问");
    let done = sessions(&port, json!({"limit": null, "offset": null, "all": true})).await;
    assert!(!done.error, "写 null 当没写，别的参数不认也不报错");
    assert_eq!(port.asked.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn no_port_failure_and_stopping() {
    let site = Site::new();
    let done = site
        .done_with_sessions("sessions", json!({}), None, UtcOffset::UTC, Stop::default())
        .await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "You have no other sessions.\n",
        "没有端口的不写第一行"
    );

    let mut port = Port::new(Vec::new());
    port.listed = Err("the core is shutting down".to_string());
    let done = sessions(&Arc::new(port), json!({})).await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "Could not list the sessions: the core is shutting down\n"
    );
    assert_eq!(
        done.human,
        Some(said("sessions/failed").with("error", "the core is shutting down"))
    );

    let stop = Stop::default();
    stop.raise();
    let port = Arc::new(Port::new((1..=2).map(nth).collect())) as Arc<dyn SessionsPort>;
    let done = site
        .done_with_sessions("sessions", json!({}), Some(port), UtcOffset::UTC, stop)
        .await;
    assert!(done.stopped, "叫停了的交回停下");
}

#[tokio::test]
async fn every_result_is_said_in_three_languages() {
    let mut checked = Vec::new();
    let port = Arc::new(Port::new(vec![nth(1)]));
    check(
        &mut checked,
        human(sessions(&port, json!({})).await),
        said("sessions/listed/one").with("count", "1"),
    );
    check(
        &mut checked,
        human(sessions(&port, json!({"offset": 1})).await),
        said("sessions/past-end/one").with("total", "1"),
    );
    let empty = Arc::new(Port::new(Vec::new()));
    check(
        &mut checked,
        human(sessions(&empty, json!({})).await),
        said("sessions/none"),
    );
    let mut failing = Port::new(Vec::new());
    failing.listed = Err("x".to_string());
    check(
        &mut checked,
        human(sessions(&Arc::new(failing), json!({})).await),
        said("sessions/failed").with("error", "x"),
    );
    readable(&checked, &["sessions"]);
    let words = Human::load(&ResourceRoot::at(resources()), "ja").expect("日文的字读得出来");
    for said in &checked {
        assert!(words.say(said).is_some(), "ja 没有 {said:?}");
    }
    let shown = words.tool("sessions").expect("ja 有显示名");
    assert_eq!(shown.subject, None, "不跟参数");
}

//! `history` 交回的给人看的说法（施工 6-4）：找到几条、读了第几到第几条、没有找到、读不了记录；读别的会话找不到、
//! 对得上不止一个、这个会话不能读别的会话（施工 C-4）；中文、英文两份字里都有，换得出字，显示名也有。

use std::sync::Arc;

use miyu_kernel::event::{Event, Said};
use miyu_kernel::id::SessionId;
use miyu_kernel::time::UtcOffset;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;
use miyu_tool::{Listing, MainSession, Opening, SessionsPort, Stop};

use crate::support::{Site, check, human, readable, resources, said};

/// 假的列会话、开日志的端口：只用来走到「找不到」「对得上不止一个」两种拒绝，一步都不会真的开日志。
struct Peers {
    this: SessionId,
    others: Vec<SessionId>,
}

fn peer(text: &str) -> MainSession {
    MainSession {
        id: SessionId::parse(text).expect("合写法"),
        title: String::new(),
        cwd: "~".to_string(),
        busy: false,
        last_active: miyu_kernel::time::Timestamp::parse("2026-09-29T05:00:00.000Z")
            .expect("合写法"),
    }
}

impl SessionsPort for Peers {
    fn this(&self) -> &SessionId {
        &self.this
    }

    fn list<'a>(&'a self, _stop: &'a Stop) -> Listing<'a> {
        let listed = self.others.iter().map(|id| peer(id.as_str())).collect();
        Box::pin(async move { Ok(listed) })
    }

    fn open<'a>(&'a self, _session: &'a SessionId) -> Opening<'a> {
        Box::pin(async { Err("not reached in this test".to_string()) })
    }
}

/// 她自己：短编号 `11112222`。
const THIS: &str = "0192f3a0-1111-7abc-8def-111122223333";
/// 短编号被两个会话共享，用来测「对得上不止一个」。
const TWIN_A: &str = "0192f3a0-2222-7abc-8def-aaaa22334455";
const TWIN_B: &str = "0192f3a0-3333-7abc-8def-bbbb22334455";

fn peers_port() -> Arc<dyn SessionsPort> {
    Arc::new(Peers {
        this: SessionId::parse(THIS).expect("合写法"),
        others: vec![
            SessionId::parse(TWIN_A).expect("合写法"),
            SessionId::parse(TWIN_B).expect("合写法"),
        ],
    })
}

/// 人说的第 `seq` 条。
fn message(seq: u64, text: &str) -> Event {
    Event::from_line(
        &serde_json::json!({
            "seq": seq, "at": "2026-09-29T05:00:00.000Z", "kind": "message.user",
            "by": {"kind": "person", "account": "alice"},
            "body": {"blocks": [{"type": "text", "text": text}]},
        })
        .to_string(),
    )
    .expect("手写的事件读得懂")
}

#[tokio::test]
async fn every_history_outcome_says_something_people_can_read() {
    let site = Site::new();
    let log = || vec![message(2, "按会话分区"), message(3, "别再用一张大表")];
    let mut checked: Vec<Said> = Vec::new();
    let run = |args: serde_json::Value| site.done_with_log("history", args, log());
    check(
        &mut checked,
        human(run(serde_json::json!({"query": "表"})).await),
        said("history/found/one").with("count", "1"),
    );
    // 找到不止一条：照旧，编号不接 `/one`（施工 4-5 再补「一个的时候说单数」）。独立的一份日志，不碰上面共用的
    // `log()`：改它会连带改掉 `history/read` 读到的范围。
    check(
        &mut checked,
        human(
            site.done_with_log(
                "history",
                serde_json::json!({"query": "会话"}),
                vec![message(2, "按会话分区"), message(3, "按会话建表")],
            )
            .await,
        ),
        said("history/found").with("count", "2"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({})).await),
        said("history/read").with("from", "2").with("to", "3"),
    );
    check(
        &mut checked,
        human(run(serde_json::json!({"query": "索引"})).await),
        said("history/none"),
    );
    check(
        &mut checked,
        human(site.done("history", serde_json::json!({})).await),
        said("history/no-log").with("error", "this call has no log"),
    );
    check(
        &mut checked,
        human(
            site.done_with_sessions(
                "history",
                serde_json::json!({"session": "deadbeef"}),
                Some(peers_port()),
                UtcOffset::UTC,
                Stop::default(),
            )
            .await,
        ),
        said("history/no-session").with("session", "deadbeef"),
    );
    check(
        &mut checked,
        human(
            site.done_with_sessions(
                "history",
                serde_json::json!({"session": "22334455"}),
                Some(peers_port()),
                UtcOffset::UTC,
                Stop::default(),
            )
            .await,
        ),
        said("history/ambiguous").with("session", "22334455"),
    );
    check(
        &mut checked,
        human(
            site.done_with_sessions(
                "history",
                serde_json::json!({"session": "22334455"}),
                None,
                UtcOffset::UTC,
                Stop::default(),
            )
            .await,
        ),
        said("history/not-here"),
    );
    readable(&checked, &["history"]);
    let words = Human::load(&ResourceRoot::at(resources()), "ja").expect("日文的字读得出来");
    for said in &checked {
        assert!(words.say(said).is_some(), "ja 没有 {said:?}");
    }
}

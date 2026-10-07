//! 切会话、补发（蓝图 `tui.md`「会话列表 `/sessions`」第 5–8 条）：带 `after` 订阅，补发来的照 [`Replay`] 读，
//! 同一个序号只办一次；启动时进最近的那个会话。

use std::io;

use serde_json::{Value, json};

use super::push::{self, Push};
use super::replay::Replay;
use super::rpc::{self, Rpc};
use super::serve::Link;
use super::sessions;

/// 带 `after` 订阅 `session`：从读到的最后一个序号往后补（没读过的从头）。记进补发中的，交回请求编号。
pub(super) async fn replay(rpc: &mut Rpc, link: &mut Link, session: &str) -> io::Result<String> {
    let after = link.seen.get(session).copied().unwrap_or(0);
    link.replays.insert(session.to_string(), Replay::default());
    let params = json!({"session": session, "stream": "events", "after": after});
    rpc.send("subscribe", params).await
}

/// 读一条推来的事件：读过的序号不再办（重新订阅两段之间会重复）；补发中的照 [`Replay`] 读。
pub(super) fn event(link: &mut Link, session: &str, event: &Value) -> Vec<Push> {
    if let Some(seq) = event["seq"].as_u64() {
        let last = link.seen.entry(session.to_string()).or_default();
        if seq <= *last {
            return Vec::new();
        }
        *last = seq;
    }
    match link.replays.get_mut(session) {
        Some(replay) => replay.read(event),
        None => push::read(event, &rpc::owns),
    }
}

/// 补完了：还记着的读出来，钟回到现在。不在补发的交回空的。
pub(super) fn finish(link: &mut Link, session: &str) -> Vec<Push> {
    link.replays
        .remove(session)
        .map(|mut replay| replay.finish())
        .unwrap_or_default()
}

/// 启动时进最近的那个会话：配置项 `ui.startup` 是 `recent`（第 8 条，核心 8-3 登记，`config.get` 读最终值）。读不出来的
/// （核心旧、没登记这一项）照 `new`。
pub(super) async fn wants_recent(rpc: &mut Rpc) -> bool {
    let asked = rpc
        .call("config.get", json!({"keys": ["ui.startup"]}))
        .await;
    asked.is_ok_and(|got| startup_is_recent(&got))
}

/// `config.get` 的回应里 `ui.startup` 是不是 `recent`。
fn startup_is_recent(got: &Value) -> bool {
    got["items"]["ui.startup"]["value"] == "recent"
}

/// 最近动静的那个主会话：有 `last_active` 的照它，没有的照核心交回的先后（新的在前）。一个都没有是 `None`。
pub(super) fn recent(list: &Value) -> Option<String> {
    let all = sessions::read(list);
    let best = all
        .iter()
        .enumerate()
        .max_by_key(|(i, s)| (s.last_active, std::cmp::Reverse(*i)))?;
    Some(best.1.session.clone())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{recent, startup_is_recent};

    #[test]
    fn only_a_recent_startup_enters_the_last_session() {
        let got =
            |v: &str| json!({"items":{"ui.startup":{"origin":{"layer":"personal"},"value":v}}});
        assert!(startup_is_recent(&got("recent")));
        assert!(!startup_is_recent(&got("new")));
        assert!(!startup_is_recent(&json!({"items":{}})), "没登记的照 new");
    }

    #[test]
    fn the_most_recent_main_session_is_the_latest_active_or_the_newest() {
        let old_core = json!({"sessions":[
            {"session":"new","parent":null},{"session":"child","parent":"new"},{"session":"old","parent":null}]});
        assert_eq!(
            recent(&old_core).as_deref(),
            Some("new"),
            "没有最近动静：照先后，新的在前"
        );
        let c3 = json!({"sessions":[
            {"session":"new","parent":null,"last_active":"2026-10-01T08:00:00Z"},
            {"session":"old","parent":null,"last_active":"2026-10-01T09:00:00Z"}]});
        assert_eq!(recent(&c3).as_deref(), Some("old"));
        assert_eq!(recent(&json!({"sessions":[]})), None);
    }
}

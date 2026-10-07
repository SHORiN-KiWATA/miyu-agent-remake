//! 会话列表（`session.list`，蓝图 `tui.md`「会话列表 `/sessions`」第 1、3 条）：读成一行行，只留主会话。
//! C-3 合进来以前没有工作目录、忙不忙、最近动静这三格，读成没有。

use serde_json::Value;

/// 列表里的一个会话。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    /// 整个编号。
    pub session: String,
    /// 标题；没起名的是 `None`。
    pub title: Option<String>,
    /// 置顶了。
    pub pinned: bool,
    /// 在哪个目录里干活（C-3）。
    pub cwd: Option<String>,
    /// 有一轮在跑（C-3）。
    pub busy: bool,
    /// 最近一次动静（C-3 的 `last_active`）。
    pub last_active: Option<jiff::Timestamp>,
    /// 没标题时核心给的预览：第一句话的第一行，最多 50 个字（核心 9-5）。
    pub preview: Option<String>,
}

/// 读 `session.list` 的回应、订阅会话列表的回应（两样一个形状）：子会话、一次性会话不要，照核心交回的先后（新的在前）。
pub fn read(result: &Value) -> Vec<SessionInfo> {
    result["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(entry)
        .collect()
}

/// 读一项；子会话、一次性会话、读不成的是 `None`。
fn entry(s: &Value) -> Option<SessionInfo> {
    if !s["parent"].is_null() || s["oneshot"] == true {
        return None;
    }
    let text = |v: &Value| v.as_str().filter(|t| !t.is_empty()).map(str::to_string);
    Some(SessionInfo {
        session: s["session"].as_str()?.to_string(),
        title: text(&s["title"]),
        pinned: s["pinned"] == true,
        cwd: s["cwd"].as_str().map(str::to_string),
        busy: s["busy"] == true,
        last_active: s["last_active"].as_str().and_then(|t| t.parse().ok()),
        preview: text(&s["preview"]),
    })
}

/// 订阅会话列表（核心 9-5）：回应是整张列表，之后推 `sessions.changed`。交回请求编号。
pub(super) async fn follow(rpc: &mut super::rpc::Rpc) -> std::io::Result<String> {
    rpc.send("subscribe", serde_json::json!({"stream": "sessions"}))
        .await
}

/// 推来的一条 `sessions.changed`（核心 9-5）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// 照编号整项换上，没有的加到最前面。
    Entry(SessionInfo),
    /// 删掉了（不列的子会话、一次性会话也照这个：列表里本来就不该有）。
    Removed(String),
}

/// 读 `sessions.changed` 的参数；读不成的是 `None`。
pub fn change(params: &Value) -> Option<Change> {
    let session = params["session"].as_str()?.to_string();
    if params["removed"] == true {
        return Some(Change::Removed(session));
    }
    Some(entry(&params["entry"]).map_or(Change::Removed(session), Change::Entry))
}

/// 照一条变化改列表。
pub fn apply(list: &mut Vec<SessionInfo>, change: Change) {
    match change {
        Change::Entry(info) => match list.iter_mut().find(|s| s.session == info.session) {
            Some(slot) => *slot = info,
            None => list.insert(0, info),
        },
        Change::Removed(session) => list.retain(|s| s.session != session),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::read;

    #[test]
    fn only_main_sessions_are_listed_and_missing_fields_read_as_none() {
        let result = json!({"sessions":[
            {"session":"a","title":"回文","pinned":true,"parent":null,"oneshot":false},
            {"session":"b","parent":"a","oneshot":false},
            {"session":"c","parent":null,"oneshot":true},
            {"session":"d","parent":null,"oneshot":false,"title":"","cwd":"/src","busy":true,
             "last_active":"2026-10-01T08:00:00Z"},
        ]});
        let list = read(&result);
        assert_eq!(
            list.iter().map(|s| s.session.as_str()).collect::<Vec<_>>(),
            ["a", "d"]
        );
        assert_eq!(list[0].title.as_deref(), Some("回文"));
        assert!(list[0].pinned && list[0].cwd.is_none() && list[0].last_active.is_none());
        assert_eq!(list[1].title, None, "空标题是没起名");
        assert!(list[1].busy);
        assert_eq!(list[1].cwd.as_deref(), Some("/src"));
        assert!(list[1].last_active.is_some());
    }

    #[test]
    fn a_pushed_change_replaces_by_id_puts_new_ones_first_and_drops_removed_ones() {
        // 核心 9-5：`sessions.changed` 照编号整项换；没标题的带 `preview`。
        use super::{Change, apply, change};
        let mut list = read(&json!({"sessions":[
            {"session":"a","title":"回文","parent":null,"oneshot":false},
            {"session":"b","parent":null,"oneshot":false,"preview":"帮我看看这个"}
        ]}));
        assert_eq!(list[1].preview.as_deref(), Some("帮我看看这个"));
        let renamed = change(
            &json!({"session":"b","entry":{"session":"b","title":"看代码","parent":null,"oneshot":false}}),
        );
        apply(&mut list, renamed.unwrap());
        assert_eq!(list[1].title.as_deref(), Some("看代码"), "整项换");
        let new = change(
            &json!({"session":"c","entry":{"session":"c","parent":null,"oneshot":false,"busy":true}}),
        );
        apply(&mut list, new.unwrap());
        assert_eq!(list[0].session, "c", "新的排到前面");
        apply(
            &mut list,
            change(&json!({"session":"a","removed":true})).unwrap(),
        );
        assert_eq!(
            list.iter().map(|s| s.session.as_str()).collect::<Vec<_>>(),
            ["c", "b"]
        );
        let child =
            change(&json!({"session":"x","entry":{"session":"x","parent":"c","oneshot":false}}));
        assert_eq!(
            child,
            Some(Change::Removed("x".into())),
            "子会话不列：当没有"
        );
    }
}

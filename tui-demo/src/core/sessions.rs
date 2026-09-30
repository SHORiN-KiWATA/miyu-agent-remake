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
}

/// 读 `session.list` 的回应：子会话、一次性会话不要，照核心交回的先后（新的在前）。
pub fn read(result: &Value) -> Vec<SessionInfo> {
    result["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["parent"].is_null() && s["oneshot"] != true)
        .filter_map(|s| {
            Some(SessionInfo {
                session: s["session"].as_str()?.to_string(),
                title: s["title"]
                    .as_str()
                    .filter(|t| !t.is_empty())
                    .map(str::to_string),
                pinned: s["pinned"] == true,
                cwd: s["cwd"].as_str().map(str::to_string),
                busy: s["busy"] == true,
                last_active: s["last_active"].as_str().and_then(|t| t.parse().ok()),
            })
        })
        .collect()
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
}

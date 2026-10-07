//! 待办（核心 D-3，`protocol.md`）：订阅的回应里带着现在的（不空的才有 `todos`），之后变了推瞬时事件 `todos.changed`；
//! 头只认这两样，不自己翻效果。两处读成同一个样子。

use serde_json::Value;

/// 待办的一项：写什么、到哪一步（`pending`、`in_progress`、`completed`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoItem {
    /// 写什么。
    pub content: String,
    /// 到哪一步。
    pub status: String,
}

/// 读 `{"todos": [...]}` 里的待办；没有这一格的是 `None`（订阅回应里待办空的不带），空列表是清空了。
pub fn read(body: &Value) -> Option<Vec<TodoItem>> {
    Some(items(body.get("todos")?.as_array()?))
}

/// 做完最后一项清空时带的刚做完的那几项（`done`，核心 D-3 补）；没有的是空的。
pub fn done(body: &Value) -> Vec<TodoItem> {
    body["done"]
        .as_array()
        .map(|l| items(l))
        .unwrap_or_default()
}

fn items(list: &[Value]) -> Vec<TodoItem> {
    list.iter()
        .map(|t| TodoItem {
            content: t["content"].as_str().unwrap_or_default().to_string(),
            status: t["status"].as_str().unwrap_or_default().to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::read;

    #[test]
    fn todos_read_from_a_push_or_a_subscribe_reply() {
        let got =
            read(&json!({"todos": [{"content": "跑测试", "status": "in_progress"}]})).unwrap();
        assert_eq!(
            (got[0].content.as_str(), got[0].status.as_str()),
            ("跑测试", "in_progress")
        );
        assert_eq!(
            read(&json!({"todos": []})),
            Some(Vec::new()),
            "空的是清空了"
        );
        assert_eq!(read(&json!({"limits": {}})), None, "订阅回应里没有：不动");
    }
}

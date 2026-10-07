//! 写过的待办（施工 D-3）：当前的是最近一份没撤掉的；撤销那一轮退回去，恢复回来；压缩以后的有效历史照样记得。

use super::Todos;
use crate::event::{
    Body, Effect, Event, Todo, TodoStatus, TodoWritten, ToolResult, TurnReverted, TurnUnreverted,
};
use crate::history::History;
use crate::id::{Seq, TurnId};
use crate::origin::By;
use crate::time::Timestamp;

fn seq(n: u64) -> Seq {
    Seq::new(n).unwrap()
}

fn event(n: u64, turn: u64, body: Body) -> Event {
    Event {
        seq: seq(n),
        at: Timestamp::parse("2026-10-07T07:00:00.000Z").unwrap(),
        turn: Some(TurnId::new(seq(turn))),
        by: By::Kernel,
        cause: None,
        body,
    }
}

fn wrote(n: u64, turn: u64, content: &str) -> Event {
    let result: ToolResult = serde_json::from_value(serde_json::json!({
        "call_id": format!("call_{}_1", n - 1),
        "status": "ok",
        "blocks": [],
    }))
    .unwrap();
    let todos = vec![Todo {
        content: content.to_string(),
        status: TodoStatus::Pending,
    }];
    event(
        n,
        turn,
        Body::ToolResult(ToolResult {
            effects: vec![Effect::TodoWritten(TodoWritten {
                todos,
                done: Vec::new(),
            })],
            ..result
        }),
    )
}

fn current(todos: &Todos) -> Option<String> {
    todos.current().map(|todos| {
        todos
            .iter()
            .map(|todo| todo.content.clone())
            .collect::<Vec<_>>()
            .join(",")
    })
}

#[test]
fn the_latest_list_not_undone_is_current() {
    let mut todos = Todos::default();
    assert_eq!(current(&todos), None, "没写过的没有");
    todos.note(&wrote(5, 3, "a"));
    todos.note(&wrote(9, 7, "b"));
    assert_eq!(current(&todos).as_deref(), Some("b"));
    let reverted: TurnReverted = serde_json::from_value(serde_json::json!({"turns": [7]})).unwrap();
    todos.note(&event(10, 7, Body::TurnReverted(reverted)));
    assert_eq!(current(&todos).as_deref(), Some("a"), "撤掉第 7 轮写的");
    let unreverted: TurnUnreverted =
        serde_json::from_value(serde_json::json!({"turns": [7]})).unwrap();
    todos.note(&event(11, 7, Body::TurnUnreverted(unreverted)));
    assert_eq!(current(&todos).as_deref(), Some("b"), "恢复回来");
}

#[test]
fn history_after_a_checkpoint_still_knows_the_list() {
    let mut history = History::default();
    history.append(wrote(5, 3, "a"));
    let after = history.after(seq(5));
    assert_eq!(
        after
            .todos()
            .map(|todos| todos[0].content.clone())
            .as_deref(),
        Some("a"),
        "写它的那一条被替代掉了，清单照样在"
    );
}

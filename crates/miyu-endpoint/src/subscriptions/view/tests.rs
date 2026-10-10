//! 写出去的推送：每样变化是它自己的方法、带它的格；接着的、同一条的 `view.append` 并成一条，别的照先后一条一条（施工 9-8 下）。

use serde_json::{Value, json};

use miyu_kernel::id::{Seq, SessionId};
use miyu_kernel::time::Timestamp;
use miyu_view::{Body, Change, Entry, EntryId, Reply};

use super::flush;

fn session() -> SessionId {
    SessionId::parse("01900000-0000-7000-8000-000000000001").expect("合写法")
}

fn block(index: usize) -> EntryId {
    EntryId::block(Seq::new(4).expect("非零"), index)
}

fn append(index: usize, text: &str) -> Change {
    Change::Append {
        id: block(index),
        text: text.to_string(),
    }
}

fn written(changes: Vec<Change>) -> Vec<Value> {
    let mut changes = changes;
    let mut lines = Vec::new();
    flush(&session(), &mut changes, &mut lines);
    assert!(changes.is_empty(), "写完就清空");
    lines
        .iter()
        .map(|line| serde_json::from_str(line).expect("是 JSON"))
        .collect()
}

#[test]
fn appends_to_the_same_entry_in_a_row_become_one() {
    let lines = written(vec![append(0, "he"), append(0, "ll"), append(0, "o")]);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["method"], "view.append");
    assert_eq!(lines[0]["params"]["text"], "hello");
    assert_eq!(lines[0]["params"]["id"], json!(block(0).as_str()));
}

#[test]
fn other_entries_and_other_changes_keep_their_order() {
    let lines = written(vec![
        append(0, "a"),
        append(1, "b"),
        append(0, "c"),
        Change::Hidden {
            ids: vec![block(0)],
            hidden: true,
        },
        append(0, "d"),
    ]);
    let methods: Vec<&str> = lines
        .iter()
        .filter_map(|line| line["method"].as_str())
        .collect();
    assert_eq!(
        methods,
        [
            "view.append",
            "view.append",
            "view.append",
            "view.hidden",
            "view.append"
        ]
    );
    assert_eq!(lines[2]["params"]["text"], "c", "隔了别的不并");
}

fn reply(index: usize) -> Entry {
    Entry {
        id: block(index),
        body: Body::Reply(Reply {
            text: "好。".to_string(),
            open: false,
        }),
        turn: None,
        hidden: false,
        at: Timestamp::from_unix_millis(0).expect("合法的时刻"),
    }
}

#[test]
fn each_change_is_its_own_method_with_its_fields() {
    let lines = written(vec![
        Change::Add {
            entry: reply(0),
            after: None,
        },
        Change::Update {
            entry: reply(0),
            after: None,
        },
        Change::Update {
            entry: reply(0),
            after: Some(Some(block(1))),
        },
        Change::Hidden {
            ids: vec![block(0)],
            hidden: false,
        },
        Change::Remove { id: block(0) },
    ]);
    let id = json!(block(0).as_str());
    assert_eq!(lines[0]["method"], "view.add");
    assert_eq!(lines[0]["params"]["entry"]["id"], id);
    assert_eq!(lines[0]["params"]["after"], Value::Null, "最前的写 null");
    assert_eq!(lines[1]["method"], "view.update");
    assert!(lines[1]["params"].get("after").is_none(), "位置没变的不写");
    assert_eq!(lines[2]["params"]["after"], json!(block(1).as_str()));
    assert_eq!(lines[3]["method"], "view.hidden");
    assert_eq!(lines[3]["params"]["ids"], json!([id]));
    assert_eq!(lines[3]["params"]["hidden"], false);
    assert_eq!(lines[4]["method"], "view.remove");
    assert_eq!(lines[4]["params"]["id"], id);
    for line in &lines {
        assert_eq!(line["params"]["session"], json!(session().as_str()));
        assert_eq!(line["jsonrpc"], "2.0");
    }
}

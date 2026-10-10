use serde_json::json;

use super::request::request;
use super::{Command, Level};

#[test]
fn switching_the_level_writes_only_what_changes() {
    // 「权限级别」第 2 条：工作区、开放权限写常用的那一级、关掉只读；只读只开只读。
    let (method, params) = request(Command::Level(Level::Full), "s1").unwrap();
    assert_eq!(method, "session.set_permission_level");
    assert_eq!(
        params,
        json!({"session": "s1", "level": "full", "read_only": false})
    );
    let (_, params) = request(Command::Level(Level::ReadOnly), "s1").unwrap();
    assert_eq!(params, json!({"session": "s1", "read_only": true}));
    let (_, params) = request(Command::Level(Level::Workspace), "s1").unwrap();
    assert_eq!(
        params,
        json!({"session": "s1", "level": "workspace", "read_only": false})
    );
}

#[test]
fn clear_goes_to_command_run_and_new_asks_for_nothing() {
    let (method, params) = request(Command::Run("/clear".into()), "s1").unwrap();
    assert_eq!(
        (method, params),
        ("command.run", json!({"session": "s1", "text": "/clear"}))
    );
    assert!(request(Command::New { keep: false }, "s1").is_none());
}

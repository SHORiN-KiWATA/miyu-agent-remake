//! 快照里的工具面：照名字排、读回来一样、没有工具的字节不变、造策略时拆成两份、重名的造不出。

use miyu_kernel::history::History;

use super::*;
use crate::test_support::*;

/// 参数格式带着空白：原样进快照、原样进 tools 数组。
const READ_PARAMETERS: &str = r#"{ "type": "object", "properties": {"path": {"type": "string"}} }"#;

fn entry(name: &str, access: Access, parameters: &str) -> ToolEntry {
    ToolEntry {
        name: name.to_string(),
        description: format!("The {name} tool."),
        parameters: serde_json::from_str::<RawJson>(parameters).unwrap(),
        access,
    }
}

/// 登记的先后倒过来：read 在前、edit 在后。
fn two() -> Vec<ToolEntry> {
    vec![
        entry("read", Access::Read, READ_PARAMETERS),
        entry("edit", Access::Write, r#"{"type":"object"}"#),
    ]
}

fn names<'a>(names: impl Iterator<Item = &'a String>) -> Vec<&'a str> {
    names.map(String::as_str).collect()
}

#[test]
fn the_tools_are_kept_by_name_and_read_back() {
    let snapshot = engineer().with_tools(two());
    assert_eq!(
        names(snapshot.tools.iter().map(|tool| &tool.name)),
        ["edit", "read"]
    );
    let text = String::from_utf8(snapshot.to_bytes()).unwrap();
    let face = concat!(
        r#""system":"You are a helpful software engineer.","tools":["#,
        r#"{"name":"edit","description":"The edit tool.","parameters":{"type":"object"},"access":"write"},"#,
        r#"{"name":"read","description":"The read tool.","parameters":{ "type": "object", "properties": {"path": {"type": "string"}} },"access":"read"}"#,
        r#"],"core":{"#,
    );
    assert!(text.contains(face), "{text}");
    let back = Snapshot::from_bytes(&snapshot.to_bytes()).unwrap();
    assert_eq!(back, snapshot);
    assert_eq!(back.tools[1].parameters.get(), READ_PARAMETERS);
    // 交进来的先后不影响字节。
    let mut reversed = two();
    reversed.reverse();
    assert_eq!(
        engineer().with_tools(reversed).to_bytes(),
        snapshot.to_bytes()
    );
}

#[test]
fn a_snapshot_without_tools_has_the_bytes_it_had_before() {
    let before = engineer();
    let text = String::from_utf8(before.to_bytes()).unwrap();
    assert!(!text.contains("\"tools\""), "{text}");
    let empty = engineer().with_tools(Vec::new());
    assert_eq!(empty.to_bytes(), before.to_bytes());
    assert_eq!(empty.hash(), before.hash());
    // M3 造的会话，快照里没有这一格：照旧读得回来。
    let back = Snapshot::from_bytes(&before.to_bytes()).unwrap();
    assert!(back.tools.is_empty());
}

#[test]
fn the_policy_gets_the_face_and_the_rules() {
    let policy = engineer().with_tools(two()).policy().unwrap();
    assert_eq!(names(policy.tools.keys()), ["edit", "read"]);
    let read = &policy.tools["read"];
    assert_eq!(read.access, Access::Read);
    assert_eq!(read.parameters.get(), READ_PARAMETERS);
    assert_eq!(policy.tools["edit"].access, Access::Write);
    let request = policy.assembler.assemble(&History::default());
    assert_eq!(
        names(request.tools.iter().map(|tool| &tool.name)),
        ["edit", "read"]
    );
    assert_eq!(request.tools[1].description, "The read tool.");
    assert_eq!(request.tools[1].parameters.get(), READ_PARAMETERS);
}

#[test]
fn two_tools_with_one_name_do_not_build() {
    let mut tools = two();
    tools.push(entry("read", Access::Write, r#"{"type":"object"}"#));
    let snapshot = engineer().with_tools(tools);
    let error = snapshot.policy().err().unwrap();
    assert_eq!(error, BuildError::DuplicateTool("read".to_string()));
    assert_eq!(error.to_string(), r#"工具面上有两件叫 "read" 的"#);
    // 从磁盘读回来的也一样：读得回来，造不出策略。
    let back = Snapshot::from_bytes(&snapshot.to_bytes()).unwrap();
    assert_eq!(
        back.policy().err().unwrap(),
        BuildError::DuplicateTool("read".to_string())
    );
}

#[test]
fn the_two_run_texts_name_the_tool() {
    let texts = engineer().run_texts().unwrap();
    assert_eq!(
        texts.unavailable("read"),
        "The tool \"read\" is not available right now.\n"
    );
    assert_eq!(
        texts.crashed("read"),
        "The tool \"read\" stopped because of an internal error. It may have been partly done.\n"
    );
    // 名字照样转义：写不出引号和尖括号。
    assert!(!texts.unavailable("a\"<b>").contains("<b>"));
}

#[test]
fn a_broken_run_text_is_named() {
    let mut broken = engineer();
    broken.core.tool_results.crashed = "The tool {nope} broke.".to_string();
    let error = broken.run_texts().unwrap_err();
    assert!(
        matches!(
            error,
            BuildError::Texts {
                which: "执行器替工具写的两句",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_snapshot_from_before_the_run_texts_reads_back_with_them_empty() {
    let text = String::from_utf8(engineer().to_bytes()).unwrap();
    let old = strip(&strip(&text, "unavailable"), "crashed");
    assert!(!old.contains("\"unavailable\""), "{old}");
    assert!(!old.contains("\"crashed\""), "{old}");
    let back = Snapshot::from_bytes(old.as_bytes()).unwrap();
    assert_eq!(back.core.tool_results.unavailable, "");
    assert_eq!(back.core.tool_results.crashed, "");
    assert_eq!(back.run_texts().unwrap().unavailable("read"), "");
}

/// 从快照的 JSON 里去掉 `field` 那一格（值是一个字符串）：造出这一格以前的样子。
fn strip(text: &str, field: &str) -> String {
    let key = format!(",\"{field}\":\"");
    let start = text.find(&key).unwrap();
    let value_start = start + key.len();
    let mut end = value_start;
    let bytes = text.as_bytes();
    while bytes[end] != b'"' || bytes[end - 1] == b'\\' {
        end += 1;
    }
    format!("{}{}", &text[..start], &text[end + 1..])
}

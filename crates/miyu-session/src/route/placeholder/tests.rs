//! `fill`（施工 8-14 补）：缺的补上、照名字排，一件不缺的一个字节都不动。

use miyu_kernel::raw::RawJson;
use miyu_kernel::request::{Request, ToolSpec};

use super::{fill, missing};

fn request(tools: Vec<ToolSpec>) -> Request {
    Request {
        tools,
        system: String::new(),
        messages: Vec::new(),
        stable: 0,
        continuation: false,
        described: Default::default(),
    }
}

fn tool(name: &str, description: &str) -> ToolSpec {
    let parameters: RawJson =
        serde_json::from_str(r#"{"type":"object","properties":{}}"#).expect("读得进");
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
    }
}

fn placeholders() -> Vec<(String, String)> {
    vec![
        ("shell".to_string(), "占位".to_string()),
        ("read".to_string(), "占位".to_string()),
    ]
}

#[test]
fn missing_placeholders_are_added_in_name_order() {
    let mut request = request(vec![tool("edit", "改文件")]);
    fill(&mut request, &placeholders());
    let names: Vec<&str> = request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    assert_eq!(names, ["edit", "read", "shell"]);
    assert_eq!(request.tools[1].description, "占位");
    assert_eq!(
        request.tools[1].parameters.get(),
        r#"{"type":"object","properties":{}}"#
    );
}

#[test]
fn a_face_that_already_has_them_is_left_alone() {
    let before = request(vec![tool("read", "读"), tool("shell", "命令")]);
    let mut after = before.clone();
    fill(&mut after, &placeholders());
    assert_eq!(after, before, "一个字节都不动");
}

#[test]
fn an_empty_list_changes_nothing() {
    let before = request(vec![tool("edit", "改文件")]);
    let mut after = before.clone();
    fill(&mut after, &[]);
    assert_eq!(after, before);
}

#[test]
fn missing_says_whether_anything_would_be_added() {
    let one = request(vec![tool("read", "读")]);
    assert!(missing(&one, &placeholders()));
    let complete = request(vec![tool("read", "读"), tool("shell", "命令")]);
    assert!(!missing(&complete, &placeholders()));
    assert!(!missing(&one, &[]), "没点名的不缺");
}

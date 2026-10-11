//! 工具、内容照 MCP 的写法读；缺了要的格子的不收或者原样留着。

use serde_json::json;

use super::*;

#[test]
fn a_tool_is_read_with_its_hints() {
    let raw = json!({
        "name": "create_issue",
        "title": "Create issue",
        "description": "Make one.",
        "inputSchema": {"type": "object"},
        "annotations": {"readOnlyHint": false, "openWorldHint": true},
    });
    let tool = Tool::read(&raw).expect("合写法");
    assert_eq!(tool.name, "create_issue");
    assert_eq!(tool.title.as_deref(), Some("Create issue"));
    assert_eq!(tool.description.as_deref(), Some("Make one."));
    assert_eq!(tool.input_schema, json!({"type": "object"}));
    assert_eq!(
        tool.annotations,
        Annotations {
            read_only: Some(false),
            destructive: None,
            idempotent: None,
            open_world: Some(true),
        }
    );
    assert_eq!(tool.raw, raw);
}

#[test]
fn a_tool_needs_a_name_and_an_object_schema() {
    assert!(Tool::read(&json!({"inputSchema": {}})).is_err());
    assert!(Tool::read(&json!({"name": "", "inputSchema": {}})).is_err());
    assert!(Tool::read(&json!({"name": "x"})).is_err());
    assert!(Tool::read(&json!({"name": "x", "inputSchema": "object"})).is_err());
}

#[test]
fn content_is_read_by_its_type() {
    let called = Called::read(&json!({
        "content": [
            {"type": "text", "text": "hi"},
            {"type": "image", "data": "aGk=", "mimeType": "image/png"},
            {"type": "audio", "data": "aGk=", "mimeType": "audio/wav"},
            {"type": "resource_link", "uri": "file:///a", "name": "a"},
            {"type": "resource", "resource": {"uri": "file:///b", "mimeType": "text/plain", "text": "b"}},
            {"type": "resource", "resource": {"uri": "file:///c", "blob": "aGk="}},
            {"type": "image", "data": "aGk="},
            {"type": "video", "url": "x"},
        ],
        "structuredContent": {"n": 1},
        "isError": true,
    }));
    assert_eq!(
        called.content,
        vec![
            Content::Text("hi".to_string()),
            Content::Image {
                data: "aGk=".to_string(),
                mime: "image/png".to_string(),
            },
            Content::Audio {
                data: "aGk=".to_string(),
                mime: "audio/wav".to_string(),
            },
            Content::Link {
                uri: "file:///a".to_string(),
                name: Some("a".to_string()),
            },
            Content::Resource {
                uri: "file:///b".to_string(),
                mime: Some("text/plain".to_string()),
                text: Some("b".to_string()),
                blob: None,
            },
            Content::Resource {
                uri: "file:///c".to_string(),
                mime: None,
                text: None,
                blob: Some("aGk=".to_string()),
            },
            Content::Other(json!({"type": "image", "data": "aGk="})),
            Content::Other(json!({"type": "video", "url": "x"})),
        ]
    );
    assert_eq!(called.structured, Some(json!({"n": 1})));
    assert!(called.is_error);
}

#[test]
fn a_bare_result_is_empty_and_not_an_error() {
    let called = Called::read(&json!({"content": "nope", "structuredContent": null}));
    assert!(called.content.is_empty());
    assert_eq!(called.structured, None);
    assert!(!called.is_error);
}

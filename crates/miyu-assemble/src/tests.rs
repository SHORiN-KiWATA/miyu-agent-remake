//! 组装器的测试：稳定区排在最前，工具面照名字排，`stable` 是示范对话的条数。

use miyu_kernel::raw::RawJson;

use super::*;
use crate::test_support::*;

fn tool(name: &str) -> ToolSpec {
    let parameters: RawJson = serde_json::from_str(r#"{"type":"object"}"#).unwrap();
    ToolSpec {
        name: name.to_string(),
        description: format!("The {name} tool."),
        parameters,
    }
}

fn stable(tools: &[&str], demos: Vec<Message>) -> Stable {
    Stable {
        tools: tools.iter().map(|name| tool(name)).collect(),
        system: "You are Miyu.".to_string(),
        demos,
    }
}

#[test]
fn tools_are_sorted_by_name() {
    let assembler = DefaultAssembler::new(stable(&["write", "read", "edit"], vec![]), texts());
    let request = assembler.assemble(Log::new().history());
    let names: Vec<&str> = request.tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["edit", "read", "write"]);
}

#[test]
fn the_demos_come_before_the_history_and_count_as_stable() {
    let demos = vec![
        Message::User {
            blocks: vec![text("你好")],
        },
        Message::Assistant {
            blocks: vec![text("你好呀")],
        },
    ];
    let assembler = DefaultAssembler::new(stable(&["read"], demos.clone()), texts());
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    let request = assembler.assemble(log.history());
    assert_eq!(request.system, "You are Miyu.");
    assert_eq!(request.stable, 2);
    assert_eq!(request.messages[..2], demos);
    assert_eq!(shape(&request.messages[2..]), ["user: hi"]);
}

#[test]
fn without_demos_nothing_is_stable() {
    let assembler = DefaultAssembler::new(stable(&["read"], vec![]), texts());
    let request = assembler.assemble(Log::new().history());
    assert_eq!(request.stable, 0);
    assert!(request.messages.is_empty());
}

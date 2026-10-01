//! 会话开局时填上能选的几个（施工 8-8 补，`docs/blueprint/policy.md`「在哪」、`tools/subagent.md`「会话开局时拼 `pool`」）：
//! 一个都没有的拿掉这个参数、和资源去掉它一字不差；有的插 `enum`、说明一行一个；别的字节不动；照快照读回。

use miyu_kernel::tool::Access;

use super::*;

/// 和资源里 `subagent` 的参数一样写法的一份：`pool` 在最后，没有 `enum`。
const WITH_POOL: &str = r#"{"type":"object","properties":{"description":{"type":"string","description":"A short title."},"prompt":{"type":"string","description":"The task."},"pool":{"type":"string","description":"Model pool for the task. Default: your own model."}},"required":["description","prompt"]}"#;

/// 同一份拿掉 `pool`：施工 8-8 以前的样子。
const WITHOUT_POOL: &str = r#"{"type":"object","properties":{"description":{"type":"string","description":"A short title."},"prompt":{"type":"string","description":"The task."}},"required":["description","prompt"]}"#;

fn entry(parameters: &str) -> ToolEntry {
    ToolEntry {
        name: "subagent".to_string(),
        description: "Start a subagent.".to_string(),
        parameters: serde_json::from_str(parameters).expect("是 JSON"),
        access: Access::Read,
    }
}

fn choice(name: &str, description: Option<&str>) -> Choice {
    Choice {
        name: name.to_string(),
        description: description.map(str::to_string),
    }
}

#[test]
fn nothing_to_choose_drops_the_parameter_byte_for_byte() {
    let mut tool = entry(WITH_POOL);
    tool.offer("pool", &[]);
    assert_eq!(tool.parameters.get(), WITHOUT_POOL);
    assert_eq!(tool.description, "Start a subagent.", "说明不动");
    assert!(tool.offered("pool").is_empty());
}

#[test]
fn choices_go_into_an_enum_and_one_line_each() {
    let mut tool = entry(WITH_POOL);
    tool.offer(
        "pool",
        &[
            choice("fast", Some("Small model for quick lookups.")),
            choice("flagship", None),
        ],
    );
    let wanted = WITH_POOL.replace(
        r#""pool":{"type":"string","description":"Model pool for the task. Default: your own model."}"#,
        r#""pool":{"type":"string","enum":["fast","flagship"],"description":"Model pool for the task. Default: your own model.\nfast: Small model for quick lookups.\nflagship"}"#,
    );
    assert_eq!(tool.parameters.get(), wanted, "只变 pool 那一格");
    assert_eq!(
        tool.offered("pool"),
        ["fast", "flagship"],
        "读回的和拼进去的一样"
    );
}

#[test]
fn a_tool_without_the_parameter_is_left_alone() {
    let mut tool = entry(WITHOUT_POOL);
    tool.offer("pool", &[choice("fast", None)]);
    assert_eq!(tool.parameters.get(), WITHOUT_POOL);
    assert!(tool.offered("pool").is_empty());
    // 以前造的快照：`tier` 带着 `enum`，读 `pool` 读不出。
    let tiered = entry(
        r#"{"type":"object","properties":{"tier":{"type":"string","enum":["lite","cheap"]}}}"#,
    );
    assert!(tiered.offered("pool").is_empty());
    assert_eq!(tiered.offered("tier"), ["lite", "cheap"]);
}

#[test]
fn odd_schemas_are_kept_as_they_are() {
    for odd in [
        r#"{"type":"object"}"#,
        r#"{"type":"object","properties":[]}"#,
        r#"{"type":"object","properties":{"pool":"x"}}"#,
        r#"[]"#,
    ] {
        let mut tool = entry(odd);
        tool.offer("pool", &[choice("fast", None)]);
        assert_eq!(tool.parameters.get(), odd, "{odd}");
        assert!(tool.offered("pool").is_empty(), "{odd}");
    }
    let numbers = entry(r#"{"type":"object","properties":{"pool":{"enum":[1,2]}}}"#);
    assert!(numbers.offered("pool").is_empty(), "enum 里不是字的读不出");
}

#[test]
fn a_parameter_without_a_description_gets_the_lines_alone() {
    let mut tool = entry(r#"{"type":"object","properties":{"pool":{"type":"string"}}}"#);
    tool.offer(
        "pool",
        &[choice("fast", Some("Quick.")), choice("slow", None)],
    );
    assert_eq!(
        tool.parameters.get(),
        r#"{"type":"object","properties":{"pool":{"type":"string","enum":["fast","slow"],"description":"fast: Quick.\nslow"}}}"#
    );
}

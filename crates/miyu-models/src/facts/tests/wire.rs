//! 照目录怎么说话（施工 8-14）：只取第 1、2 层，能不能关思考照模型真走的驱动。

use serde_json::json;

use super::*;

/// 第 1、2 层对上的取模型自己的包名、交错思考的字段；第 3、4 层按名字对上的不取；能不能关思考照模型真走的驱动算。
#[test]
fn the_wire_comes_only_from_the_first_two_layers() {
    let held = Held::new(
        json!({"npm": {"@ai-sdk/openai-compatible": "openai-chat", "@ai-sdk/anthropic": "anthropic",
                       "@ai-sdk/openai": "openai-responses"}}),
        true,
    );
    let go = "[providers.opencode-go]\nlocal = false\n";
    let (flash, _) = facts_of(&held, go, "opencode-go", "deepseek-v4.1-flash");
    assert_eq!(
        flash.wire,
        Wire {
            npm: None,
            interleaved: Some("reasoning_content".to_string())
        }
    );
    let (minimax, _) = facts_of(&held, go, "opencode-go", "minimax-m3");
    assert_eq!(minimax.wire.npm.as_deref(), Some("@ai-sdk/anthropic"));
    assert_eq!(
        minimax.reasoning.value,
        Some(vec!["off".to_string(), "on".to_string()]),
        "走 anthropic：开关是接口自带的"
    );
    // 手写指定（第 1 层）也取。
    let pointed = "[providers.relay]\ndriver = \"openai-chat\"\nbase_url = \"https://relay.invalid/v1\"\n\n[providers.relay.models.m]\ncatalog = \"opencode-go/minimax-m3\"\n";
    let (pointed, _) = facts_of(&held, pointed, "relay", "m");
    assert_eq!(pointed.wire.npm.as_deref(), Some("@ai-sdk/anthropic"));
    // 认不出的中转按名字对上（第 3 层）：不取。
    let relay =
        "[providers.relay]\ndriver = \"openai-chat\"\nbase_url = \"https://relay.invalid/v1\"\n";
    let (named, found) = facts_of(&held, relay, "relay", "deepseek-v4.1-flash");
    assert!(
        matches!(&found, Found::Matched(matched) if matched.layer == 3),
        "{found:?}"
    );
    assert_eq!(named.wire, Wire::default());
    let (minimax, found) = facts_of(&held, relay, "relay", "minimax-m3");
    assert!(
        matches!(&found, Found::Matched(matched) if matched.layer == 3),
        "{found:?}"
    );
    assert_eq!(minimax.wire, Wire::default());
    assert_eq!(
        minimax.reasoning.value, None,
        "走 openai-chat、没有开关：只有开关的模型没有档位"
    );
}

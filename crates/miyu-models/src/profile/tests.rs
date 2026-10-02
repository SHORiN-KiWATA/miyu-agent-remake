//! 档案的读法（施工 8-6）：几格的写法、一格格盖在驱动的默认上、不认识的格读不进来；`[npm]` 的表（施工 8-7）。

use serde_json::json;

use super::*;

#[test]
fn the_deepseek_profile_reads_into_the_switches() {
    let profiles = Profiles::parse(&json!({"providers": {"deepseek": {
        "driver": "openai-chat",
        "base_url": "https://api.deepseek.com",
        "image_tokens": "deepseek",
        "compat": {
            "reasoning": {"replay": "reasoning_content", "always": true},
            "continuation": {"field": "prefix", "path": "/beta/chat/completions"},
            "toggle": {"field": "thinking", "on": {"type": "enabled"}, "off": {"type": "disabled"}}
        }
    }}}))
    .expect("读得进来");
    let deepseek = &profiles.providers["deepseek"];
    assert_eq!(deepseek.driver.as_deref(), Some("openai-chat"));
    assert_eq!(deepseek.image_tokens, Some(ImageTokens::DeepSeek));
    let compat = deepseek.compat.as_ref().expect("有开关").compat();
    assert_eq!(compat, Compat::deepseek(), "和测试用的那一套一样");
}

#[test]
fn each_switch_lays_over_the_default() {
    let compat = |spec: serde_json::Value| {
        serde_json::from_value::<CompatSpec>(spec)
            .expect("写法对")
            .compat()
    };
    assert_eq!(compat(json!({})), Compat::default());
    assert_eq!(
        compat(json!({"output_limit": "max_completion_tokens", "stream_usage": false})),
        Compat {
            output_limit: OutputLimit::MaxCompletionTokens,
            stream_usage: false,
            ..Compat::default()
        }
    );
    assert_eq!(
        compat(json!({"reasoning": "drop", "continuation": "none"})),
        Compat::default()
    );
    // 开关思考的字段（施工 8-18）：值照原样的 JSON，什么形状都收。
    assert_eq!(
        compat(json!({"toggle": {"field": "enable_thinking", "on": true, "off": false}})).toggle,
        Some(Box::new(Toggle {
            field: "enable_thinking".to_string(),
            on: json!(true),
            off: json!(false),
        }))
    );
    assert_eq!(
        compat(
            json!({"reasoning": {"replay": "reasoning", "always": false},
                       "continuation": {"field": "partial", "path": "/p"}})
        ),
        Compat {
            reasoning: ReasoningReplay::Replay {
                field: ReasoningField::Reasoning,
                always: false
            },
            continuation: Continuation::Prefix {
                field: ContinuationField::Partial,
                path: "/p".to_string()
            },
            ..Compat::default()
        }
    );
}

#[test]
fn a_profile_with_a_stray_field_is_not_read() {
    for bad in [
        json!({"providers": {"x": {"drivers": "openai-chat"}}}),
        json!({"providers": {"x": {"inputs": ["text"]}}}),
        json!({"providers": {"x": {"compat": {"reasoning": "keep"}}}}),
        json!({"providers": {"x": {"image_tokens": "other"}}}),
        json!({"npm": {"@ai-sdk/openai-compatible": 1}}),
        json!({"vendors": {}}),
    ] {
        let error = Profiles::parse(&bad).expect_err("读不进来");
        assert!(
            error.starts_with("models/profiles.toml not readable: "),
            "{error}"
        );
    }
}

/// `[npm]`：AI SDK 包名 → 驱动（施工 8-7）。
#[test]
fn the_npm_table_names_drivers() {
    let profiles = Profiles::parse(&json!({"npm": {"@ai-sdk/openai-compatible": "openai-chat"}}))
        .expect("读得进来");
    assert_eq!(profiles.npm["@ai-sdk/openai-compatible"], "openai-chat");
    assert!(profiles.providers.is_empty());
}

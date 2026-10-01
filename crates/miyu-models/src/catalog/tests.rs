//! 目录怎么读（施工 8-7）：真目录裁出来的一份读得进、坏的模型跳过、不认识的格不理、`limit.input` 比 `context` 小的取小的。

use serde_json::json;

use super::*;
use crate::test_support::TRIMMED;

#[test]
fn the_trimmed_real_catalog_reads_with_the_broken_model_skipped() {
    let read = Catalog::parse(TRIMMED).expect("读得进");
    assert_eq!(read.skipped, ["deepseek/deepseek-broken"]);
    let catalog = read.catalog;
    assert_eq!(catalog.providers().count(), 10);
    assert_eq!(catalog.model_count(), 19);
    let deepseek = catalog.provider("deepseek").expect("有 DeepSeek");
    assert_eq!(deepseek.name.as_deref(), Some("DeepSeek"));
    assert_eq!(deepseek.env, ["DEEPSEEK_API_KEY"]);
    assert_eq!(deepseek.npm.as_deref(), Some("@ai-sdk/openai-compatible"));
    assert_eq!(deepseek.api.as_deref(), Some("https://api.deepseek.com"));
    assert!(deepseek.doc.is_some());
    // 用得上的格一格格读对；不认识的（`description`、`release_date` 这些）不理。
    let flash = catalog.model("deepseek", "deepseek-flash").expect("有");
    assert_eq!(flash.name.as_deref(), Some("DeepSeek V4.1 Flash"));
    assert_eq!(flash.family.as_deref(), Some("deepseek-flash"));
    assert_eq!(flash.tools, Some(true));
    assert_eq!(
        flash.inputs,
        Some(vec!["text".to_string(), "image".to_string()])
    );
    assert_eq!(
        (flash.window, flash.max_output),
        (Some(1_000_000), Some(393_216))
    );
    assert_eq!(
        flash.reasoning,
        Some(Reasoning {
            levels: vec!["low".to_string(), "high".to_string(), "max".to_string()],
            toggle: true,
        })
    );
    assert_eq!(flash.status, None);
    let price = flash.price.as_ref().expect("有价");
    assert_eq!(price.rates.input, Some(0.15));
    assert_eq!(price.rates.cache_write, None);
    assert_eq!(price.reasoning, Some(0.6));
    assert_eq!(price.currency, "USD");
    assert_eq!(
        catalog
            .model("deepseek", "deepseek-v4-flash")
            .and_then(|model| model.status.as_deref()),
        Some("deprecated")
    );
    // 窗口取 `limit.context` 和 `limit.input` 里小的那个。
    let hy3 = catalog.model("deepinfra", "tencent/Hy3").expect("有");
    assert_eq!((hy3.window, hy3.max_output), (Some(192_000), Some(128_000)));
    assert_eq!(hy3.reasoning, None, "空的 reasoning_options 是没有");
    // 分档、超过 20 万的价。
    let sonnet = catalog.model("aihubmix", "claude-sonnet-4-5").expect("有");
    let price = sonnet.price.as_ref().expect("有价");
    assert_eq!(price.tiers.len(), 1);
    assert_eq!(price.tiers[0].size, 200_000);
    assert_eq!(price.tiers[0].rates.input, Some(6.6));
    assert_eq!(price.over_200k.and_then(|rates| rates.output), Some(24.75));
}

#[test]
fn the_index_finds_names_and_normalized_names_in_byte_order() {
    let catalog = Catalog::parse(TRIMMED).expect("读得进").catalog;
    let named: Vec<&str> = catalog
        .named("deepseek-v4.1-flash")
        .iter()
        .map(|(provider, _)| provider.as_str())
        .collect();
    assert_eq!(named, ["above", "aihubmix", "opencode", "opencode-go"]);
    assert_eq!(catalog.normalized("deepseek-v4-1-flash").len(), 4);
    assert_eq!(
        catalog.normalized("hy3"),
        [("deepinfra".to_string(), "tencent/Hy3".to_string())]
    );
    assert!(catalog.named("nope").is_empty());
}

#[test]
fn odd_fields_are_read_the_cautious_way() {
    let text = json!({
        "p": {"id": "p", "models": {
            "toggle": {"reasoning_options": [{"type": "toggle"}], "modalities": {"input": ["text", "audio", "pdf"]}},
            "null-level": {"reasoning_options": [{"type": "effort", "values": [null, "low", "high"]}]},
            "off-by-name": {"reasoning_options": [{"type": "effort", "values": ["none", "disabled", "low"]}]},
            "budget-only": {"reasoning_options": [{"type": "budget_tokens", "min": 1024}]},
            "input-only": {"limit": {"input": 5000}},
            "zero": {"limit": {"context": 0, "output": 0}},
            "tiered": {"cost": {"input": 1, "tiers": [{"input": 2, "tier": {"type": "other", "size": 9}}]}},
            "bad-limit": {"limit": {"context": -1}},
            "bad-cost": {"cost": {"input": "cheap"}},
            "bad-tools": {"tool_call": "yes"}
        }},
        "q": {"env": "NOT_A_LIST", "models": {}}
    })
    .to_string();
    let read = Catalog::parse(&text).expect("读得进");
    assert_eq!(
        read.skipped,
        ["p/bad-cost", "p/bad-limit", "p/bad-tools", "q"]
    );
    let model = |name: &str| read.catalog.model("p", name).cloned().expect("有");
    assert_eq!(
        model("toggle").reasoning,
        Some(Reasoning {
            levels: Vec::new(),
            toggle: true
        }),
        "只有开关的：没有档位，开关记下（能不能关照档案，施工 8-18）"
    );
    assert_eq!(
        model("off-by-name").reasoning,
        Some(Reasoning {
            levels: vec!["off".to_string(), "low".to_string()],
            toggle: false
        }),
        "none、disabled 读成 off，重复的只留第一个（施工 8-18）"
    );
    assert_eq!(
        model("budget-only").reasoning,
        None,
        "只有思考预算的没有档位"
    );
    assert_eq!(
        model("toggle").inputs,
        Some(vec!["text".to_string(), "pdf".to_string()]),
        "认得的几种照固定的先后"
    );
    assert_eq!(
        model("null-level")
            .reasoning
            .map(|reasoning| reasoning.levels),
        Some(vec!["low".to_string(), "high".to_string()]),
        "写了 null 的那一级不算，整个模型照读"
    );
    assert_eq!(model("input-only").window, Some(5000));
    assert_eq!(
        (model("zero").window, model("zero").max_output),
        (None, None)
    );
    let tiered = model("tiered").price.expect("有价");
    assert!(tiered.tiers.is_empty(), "不认识的档不理");
    assert_eq!(model("tiered").inputs, None);
}

#[test]
fn a_catalog_that_is_not_a_table_of_providers_is_unreadable() {
    for text in ["", "[]", "{\"p\": 1", "not json"] {
        let error = Catalog::parse(text).expect_err("读不了");
        assert!(error.starts_with("catalog not readable: "), "{error}");
    }
    assert_eq!(
        Catalog::parse("{}")
            .expect("空的也读得进")
            .catalog
            .model_count(),
        0
    );
}

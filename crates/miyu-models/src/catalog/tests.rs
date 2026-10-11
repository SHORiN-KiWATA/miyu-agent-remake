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
    assert_eq!(catalog.model_count(), 22);
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
        flash
            .inputs
            .map(|inputs| inputs.names().collect::<Vec<_>>()),
        Some(vec!["text", "image"])
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
        .map(|(provider, _)| &**provider)
        .collect();
    assert_eq!(named, ["above", "aihubmix", "opencode", "opencode-go"]);
    assert_eq!(catalog.normalized("deepseek-v4-1-flash").len(), 4);
    let hy3: Vec<(&str, &str)> = catalog
        .normalized("hy3")
        .iter()
        .map(|(provider, model)| (&**provider, &**model))
        .collect();
    assert_eq!(hy3, [("deepinfra", "tencent/Hy3")]);
    assert!(catalog.named("nope").is_empty());
}

#[test]
fn odd_fields_are_read_the_cautious_way() {
    let text = json!({
        "p": {"id": "p", "models": {
            "toggle": {"reasoning_options": [{"type": "toggle"}], "modalities": {"input": ["video", "text", "audio", "smell", "pdf"]}},
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
        model("toggle")
            .inputs
            .map(|inputs| inputs.names().collect::<Vec<_>>()),
        Some(vec!["text", "pdf", "audio", "video"]),
        "认得的几种照固定的先后，音频、视频也认（施工 8-27），认不得的不要"
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

/// 模型自己的包名、交错思考（施工 8-14）：`provider.npm`、`interleaved` 写成 `{"field": …}` 的取字段名，`true` 和别的写法当没写。
#[test]
fn a_model_carries_its_own_package_and_interleaved_field() {
    let catalog = Catalog::parse(TRIMMED).expect("读得进").catalog;
    let model = |provider: &str, name: &str| catalog.model(provider, name).expect("有").clone();
    assert_eq!(
        model("opencode-go", "minimax-m3").npm.as_deref(),
        Some("@ai-sdk/anthropic")
    );
    assert_eq!(
        model("opencode-go", "gpt-5.6-luna").npm.as_deref(),
        Some("@ai-sdk/openai")
    );
    assert_eq!(
        model("opencode", "gemini-3-pro").npm.as_deref(),
        Some("@ai-sdk/google")
    );
    let flash = model("opencode-go", "deepseek-v4.1-flash");
    assert_eq!(flash.npm, None, "没写的照这一家的");
    assert_eq!(flash.interleaved.as_deref(), Some("reasoning_content"));
    assert_eq!(model("opencode-go", "minimax-m3").interleaved, None);
    assert_eq!(
        model("aihubmix", "claude-sonnet-4-5").interleaved,
        None,
        "写成 true 的说不出字段"
    );
    let odd = Catalog::parse(
        &json!({"p": {"models": {
            "details": {"interleaved": {"field": "reasoning_details"}},
            "number": {"interleaved": {"field": 3}},
            "no-npm": {"provider": {"api": "https://x.invalid"}}
        }}})
        .to_string(),
    )
    .expect("读得进");
    assert!(odd.skipped.is_empty(), "{:?}", odd.skipped);
    let odd = odd.catalog;
    assert_eq!(
        odd.model("p", "details")
            .expect("有")
            .interleaved
            .as_deref(),
        Some("reasoning_details"),
        "认不认交给合资料的一方"
    );
    assert_eq!(odd.model("p", "number").expect("有").interleaved, None);
    assert_eq!(odd.model("p", "no-npm").expect("有").npm, None);
}

#[test]
fn the_same_text_is_kept_once() {
    // 同样的字只存一份（施工 V-2 下）：两家列了同一个模型，表里的键和索引里的条目是同一份；家族照字共用。
    let catalog = Catalog::parse(TRIMMED).expect("读得进").catalog;
    let entries = catalog.named("deepseek-v4.1-flash");
    let (provider, model) = &entries[0];
    let key = catalog
        .provider(provider)
        .and_then(|entry| entry.models.keys().find(|name| ***name == **model))
        .expect("在表里");
    assert!(
        std::sync::Arc::ptr_eq(key, model),
        "索引里的名字就是表里的键"
    );
    let families: Vec<_> = entries
        .iter()
        .filter_map(|(provider, model)| catalog.model(provider, model)?.family.clone())
        .collect();
    assert!(families.len() >= 2, "{families:?}");
    assert!(
        families
            .windows(2)
            .all(|pair| *pair[0] != *pair[1] || std::sync::Arc::ptr_eq(&pair[0], &pair[1])),
        "同样的家族是同一份"
    );
}

#[test]
fn inputs_are_switches() {
    let inputs = super::Inputs::of(["video", "text", "smell", "image"]);
    assert_eq!(
        inputs.names().collect::<Vec<_>>(),
        ["text", "image", "video"]
    );
    assert!(inputs.has("image") && !inputs.has("pdf") && !inputs.has("smell"));
    assert!(super::Inputs::of([]).is_empty());
}

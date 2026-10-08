//! 温度（施工 8-22）：驱动的上限、这个模型用不用得了、资料里的两格、配置里写的用不了的。

use miyu_config::parse::parse;
use serde_json::{Value as Json, json};

use super::*;
use crate::facts::{Source, facts};
use crate::provider::provider;
use crate::settings::ModelSettings;
use crate::test_support::{Held, items, resolved};

/// 手写的写成系统配置。
fn file(layer: Layer) -> String {
    format!("{}/config.toml", layer.as_str())
}

#[test]
fn anthropic_takes_up_to_one_and_the_others_up_to_two() {
    assert_eq!(ceiling(Driver::Anthropic), 1.0);
    assert_eq!(ceiling(Driver::OpenAiChat), 2.0);
    assert_eq!(ceiling(Driver::OpenAiResponses), 2.0);
}

#[test]
fn a_model_the_catalog_says_takes_none_cannot_have_one() {
    assert_eq!(
        usable(0.7, Some(false), Driver::OpenAiChat),
        Err(Unusable::Unsupported)
    );
    assert_eq!(usable(0.7, Some(true), Driver::OpenAiChat), Ok(0.7));
    assert_eq!(
        usable(0.7, None, Driver::OpenAiChat),
        Ok(0.7),
        "目录没说的当能调"
    );
}

#[test]
fn above_the_driver_ceiling_is_unusable_and_the_ceiling_itself_is_fine() {
    assert_eq!(usable(1.0, None, Driver::Anthropic), Ok(1.0));
    assert_eq!(
        usable(1.2, Some(true), Driver::Anthropic),
        Err(Unusable::TooHigh(1.0))
    );
    assert_eq!(usable(2.0, None, Driver::OpenAiResponses), Ok(2.0));
    assert_eq!(usable(0.0, Some(true), Driver::Anthropic), Ok(0.0));
    assert_eq!(
        usable(1.5, Some(false), Driver::Anthropic),
        Err(Unusable::Unsupported),
        "不收的先说不收"
    );
}

#[test]
fn the_item_is_the_one_the_settings_register() {
    assert!(ModelSettings::ITEMS.iter().any(|item| item.key == ITEM));
}

/// 档案里 DeepSeek 走 `openai-chat`、Anthropic 走 `anthropic`、OpenAI 走 `openai-responses`；裁出来的目录里 `deepseek-flash`、`claude-sonnet-4-5` 能调，
/// OpenAI 的 `gpt-5` 不能调。
fn held() -> Held {
    Held::new(
        json!({
            "npm": {"@ai-sdk/openai-compatible": "openai-chat", "@ai-sdk/anthropic": "anthropic", "@ai-sdk/openai": "openai-responses"},
            "providers": {
                "deepseek": {"driver": "openai-chat", "base_url": "https://api.deepseek.com"},
                "anthropic": {"driver": "anthropic", "base_url": "https://api.anthropic.com"},
                "openai": {"driver": "openai-responses", "base_url": "https://api.openai.com/v1"}
            }
        }),
        true,
    )
}

/// 照配置 `source` 查 `id` 这一家的 `model` 能不能调、走哪种驱动；那一家用不了的说不出来。
fn model_of(source: &str) -> impl Fn(&str, &str) -> Option<(Option<bool>, Driver)> {
    let resolved = resolved(source);
    let held = held();
    move |id: &str, model: &str| {
        let knowledge = held.knowledge();
        let provider = provider(&resolved.values(), &knowledge, id).ok()?;
        let (facts, _) = facts(&resolved, &knowledge, &provider, model);
        let driver = provider.driver_for(model, &facts.wire, &knowledge.profiles.npm);
        Some((facts.takes_temperature.value, driver))
    }
}

#[test]
fn a_written_temperature_the_model_cannot_use_is_reported_where_it_is_written() {
    let source = "[providers.deepseek]\nlocal = false\n\n[providers.deepseek.models.\"deepseek-flash\"]\ntemperature = 0.7\n\n[providers.openai]\nlocal = false\n\n[providers.openai.models.\"gpt-5\"]\ntemperature = 0.2\n\n[providers.anthropic]\nlocal = false\n\n[providers.anthropic.models.\"claude-sonnet-4-5\"]\ntemperature = 1.5\n\n[providers.anthropic.models.\"claude-haiku-9\"]\ntemperature = 1\n\n[providers.broken.models.x]\ntemperature = 2\n";
    let parsed = parse(&items(), Layer::System, source).expect("写法对");
    let found = unusable(&parsed, Layer::System, &model_of(source));
    assert_eq!(found.len(), 2, "{found:?}");
    let keys: Vec<_> = found.iter().map(|problem| problem.key.as_deref()).collect();
    assert_eq!(
        keys,
        [
            Some("providers.anthropic.models.claude-sonnet-4-5.temperature"),
            Some("providers.openai.models.gpt-5.temperature"),
        ]
    );
    let too_high = &found[0];
    assert_eq!(too_high.code, Code::UnusableTemperature);
    assert_eq!(too_high.code.as_str(), "unusable_temperature");
    assert_eq!(too_high.name.as_deref(), Some("1"), "超过上限的说上限");
    assert_eq!(too_high.got.as_deref(), Some("1.5"));
    assert_eq!(too_high.at.map(|at| at.line), Some(17), "指到值那一行");
    let unsupported = &found[1];
    assert_eq!(unsupported.name, None, "不收的不带上限");
    assert_eq!(unsupported.at.map(|at| at.line), Some(11));
}

#[test]
fn a_temperature_written_where_it_does_not_count_is_not_checked() {
    let source = "[providers.openai.models.\"gpt-5\"]\ntemperature = 0.2\n";
    let parsed = parse(&items(), Layer::Project, source).expect("写法对");
    let config = "[providers.openai]\nlocal = false\n";
    assert!(
        unusable(&parsed, Layer::Project, &model_of(config)).is_empty(),
        "项目配置里写的本来就不算（wrong_layer 报过了）"
    );
}

/// 温度（施工 8-22）：能不能调照目录借；默认的温度只认写了的、这个模型用得了的：目录说不收的、超过它真走的驱动的上限的
/// 照没写。目录没说的当能调。
#[test]
fn the_default_temperature_counts_only_where_the_model_can_take_it() {
    let held = held();
    let at = |id: &str, model: &str, value: &str| {
        let source = format!(
            "[providers.{id}]\nlocal = false\n\n[providers.{id}.models.\"{model}\"]\ntemperature = {value}\n"
        );
        let resolved = resolved(&source);
        let knowledge = held.knowledge();
        let provider = provider(&resolved.values(), &knowledge, id).expect("配了");
        facts(&resolved, &knowledge, &provider, model).0
    };
    let facts = at("deepseek", "deepseek-flash", "0.7");
    assert_eq!(facts.temperature.value, Some(0.7));
    assert_eq!(
        Json::Object(facts.temperature.source.json(&file)),
        json!({"from": "config", "file": "system/config.toml", "line": 5, "layer": "system"})
    );
    assert_eq!(facts.takes_temperature.value, Some(true));
    let facts = at("openai", "gpt-5", "0.2");
    assert_eq!(facts.takes_temperature.value, Some(false));
    assert!(matches!(
        facts.takes_temperature.source,
        Source::Catalog { .. }
    ));
    assert_eq!(
        (facts.temperature.value, facts.temperature.source),
        (None, Source::Default),
        "目录说不收：照没写"
    );
    let facts = at("anthropic", "claude-sonnet-4-5", "1.5");
    assert_eq!(facts.temperature.value, None, "Anthropic 的上限是 1");
    let facts = at("anthropic", "claude-sonnet-4-5", "1");
    assert_eq!(facts.temperature.value, Some(1.0), "上限本身能用");
    let facts = at("deepseek", "deepseek-broken", "2");
    assert_eq!(
        (
            facts.takes_temperature.value,
            facts.takes_temperature.source
        ),
        (None, Source::Default),
        "目录没写这一格"
    );
    assert_eq!(facts.temperature.value, Some(2.0), "目录没说的当能调");
}

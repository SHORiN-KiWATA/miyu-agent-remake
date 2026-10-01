//! 一家供应商这一轮的样子、一个引用发给谁（施工 8-6）：手写的压过档案、档案照 `catalog` 找、推不出来的说清楚、没配
//! `models.chat` 的是 `no_model` 的那一句，窗口手写的压过模型资料。

use miyu_config::merge::{Layers, merge};
use miyu_config::parse::parse;
use miyu_config::{Item, Layer};
use serde_json::json;

use super::*;
use crate::settings::{ModelSettings, ProviderSettings, UseSettings};

fn items() -> Vec<Item> {
    [
        ProviderSettings::ITEMS,
        ModelSettings::ITEMS,
        UseSettings::ITEMS,
    ]
    .concat()
}

/// 当成系统配置读、合：写错的不收。
fn values(source: &str) -> Values {
    let parsed = parse(&items(), Layer::System, source).expect("写法对");
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    let layers = Layers {
        system: Some(&parsed),
        ..Layers::default()
    };
    merge(&items(), &layers, &|_| None).values()
}

fn profiles() -> Profiles {
    Profiles::parse(&json!({"providers": {"deepseek": {
        "driver": "openai-chat",
        "base_url": "https://api.deepseek.com",
        "inputs": ["text", "image"],
        "image_tokens": "deepseek",
        "compat": {"reasoning": {"replay": "reasoning_content", "always": true}}
    }}}))
    .expect("读得进来")
}

fn table() -> ModelTable {
    ModelTable::parse(
        r#"{"source":"x","fetched":"y","providers":{"deepseek":{"models":{"deepseek-flash":{"limit":{"context":1000000,"output":393216}}}}}}"#,
    )
    .expect("读得进来")
}

#[test]
fn a_known_provider_needs_only_its_keys() {
    let values = values("[providers.deepseek]\nkeys = [{ env = \"DEEPSEEK_API_KEY\" }]\n");
    let deepseek = provider(&values, &profiles(), "deepseek").expect("档案推得出");
    assert_eq!(deepseek.driver, Driver::OpenAiChat);
    assert_eq!(deepseek.base_url, "https://api.deepseek.com");
    assert_eq!(deepseek.keys, [KeyRef::Env("DEEPSEEK_API_KEY".to_string())]);
    assert!(deepseek.inputs.images);
    assert_eq!(deepseek.images, Some(ImageTokens::DeepSeek));
    assert_ne!(deepseek.compat, Compat::default(), "开关照档案");
    assert_eq!(deepseek.catalog, "deepseek");
}

#[test]
fn hand_written_values_win_and_the_profile_is_found_by_catalog() {
    let values = values(
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"https://relay.invalid/v1\"\ncatalog = \"deepseek\"\n\n[providers.plain]\ndriver = \"openai-chat\"\nbase_url = \"http://plain.invalid:9/v1\"\n",
    );
    let dev = provider(&values, &profiles(), "dev").expect("手写的");
    assert_eq!(dev.base_url, "https://relay.invalid/v1", "手写的压过档案");
    assert_eq!(
        dev.compat,
        profiles().providers["deepseek"]
            .compat
            .as_ref()
            .map(|c| c.compat())
            .unwrap()
    );
    assert_eq!(dev.catalog, "deepseek");
    assert!(dev.keys.is_empty(), "没写 key 的不带认证头");
    let plain = provider(&values, &profiles(), "plain").expect("手写的");
    assert_eq!(
        (plain.compat, plain.inputs, plain.images),
        (Compat::default(), Inputs::default(), None),
        "没有档案的照驱动的默认"
    );
}

#[test]
fn a_provider_that_cannot_be_worked_out_says_why() {
    let values = values(
        "[providers.newapi]\nkeys = []\n\n[providers.claude]\ndriver = \"anthropic\"\nbase_url = \"https://a.invalid\"\n",
    );
    assert_eq!(
        provider(&values, &profiles(), "newapi"),
        Err(NoModel(
            r#"provider "newapi" needs driver and base_url: it matches nothing in the catalog"#
                .to_string()
        ))
    );
    assert_eq!(
        provider(&values, &profiles(), "claude"),
        Err(NoModel(
            r#"driver "anthropic" of provider "claude" is not available yet"#.to_string()
        ))
    );
    assert_eq!(
        provider(&values, &profiles(), "deepseek"),
        Err(NoModel(r#"no provider "deepseek""#.to_string())),
        "档案认得也要配了才算"
    );
}

#[test]
fn a_reference_resolves_to_a_provider_and_a_model() {
    let values = values(
        "[providers.deepseek]\nkeys = [{ secret = \"deepseek\" }]\n\n[models]\nchat = \"deepseek/deepseek-flash\"\n",
    );
    let found = target(&values, &profiles(), "deepseek/deepseek-flash").expect("解析得出");
    assert_eq!(
        (found.provider.id.as_str(), found.model.as_str()),
        ("deepseek", "deepseek-flash")
    );
    let tier = target(&values, &profiles(), "flagship").expect("挡位没配退回 chat");
    assert_eq!(tier, found);
    assert_eq!(
        target(&values, &profiles(), "@free"),
        Err(NoModel(r#"no pool "free""#.to_string()))
    );
    assert_eq!(
        target(&values, &profiles(), "openai/gpt-5"),
        Err(NoModel(r#"no provider "openai""#.to_string()))
    );
    assert_eq!(
        target(&values, &profiles(), "nope"),
        Err(NoModel(
            r#""nope" is not a model, a pool or a tier"#.to_string()
        ))
    );
    assert_eq!(chat(&values).as_deref(), Some("deepseek/deepseek-flash"));
    let empty = Values::default();
    assert_eq!(chat(&empty), None);
    assert_eq!(
        target(&empty, &profiles(), "lite"),
        Err(NoModel(NOT_CONFIGURED.to_string()))
    );
    assert_eq!(NOT_CONFIGURED, "no model configured: set models.chat");
}

#[test]
fn a_written_window_wins_over_the_model_data() {
    let values = values(
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"https://relay.invalid\"\ncatalog = \"deepseek\"\n\n[providers.dev.models.\"deepseek-flash\"]\nwindow = 60000\n\n[providers.dev.models.\"v4.1\"]\nwindow = 128000\n",
    );
    let at = |model: &str| target(&values, &profiles(), &format!("dev/{model}")).expect("解析得出");
    assert_eq!(
        facts(&values, &table(), &at("deepseek-flash")),
        ModelFacts {
            window: Some(60_000),
            max_output: Some(393_216)
        },
        "照 catalog 那一家查，窗口手写的压过"
    );
    assert_eq!(
        facts(&values, &table(), &at("v4.1")),
        ModelFacts {
            window: Some(128_000),
            max_output: None
        },
        "模型名里有点的照样认"
    );
    assert_eq!(
        facts(&values, &table(), &at("other")),
        ModelFacts::default()
    );
}

//! 模型的资料（施工 8-7）：每一格各查各的，价格整份不拼，第 3、4 层借哪些，本机的当免费，倍率谁盖谁，来源一字不差。

use miyu_config::Layer;
use miyu_kernel::time::Timestamp;
use serde_json::json;

use super::*;
use crate::observed::{ListedModel, ProviderList};
use crate::provider::provider;
use crate::test_support::{Held, resolved};

/// 档案只认 OpenAI 兼容的包，带上裁出来的目录。
fn held() -> Held {
    Held::new(
        json!({"npm": {"@ai-sdk/openai-compatible": "openai-chat"}}),
        true,
    )
}

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("时刻写法对")
}

/// 照配置 `source` 查 `id` 这一家的 `model`。
fn facts_of(held: &Held, source: &str, id: &str, model: &str) -> (Facts, Found) {
    let resolved = resolved(source);
    let knowledge = held.knowledge();
    let provider = provider(&resolved.values(), &knowledge, id).expect("配了");
    facts(&resolved, &knowledge, &provider, model)
}

/// 手写的写成系统配置。
fn file(layer: Layer) -> String {
    format!("{}/config.toml", layer.as_str())
}

const DEEPSEEK: &str = "[providers.deepseek]\nkeys = []\n";

#[test]
fn every_fact_from_the_catalog_carries_its_entry_layer_and_date() {
    let (facts, found) = facts_of(&held(), DEEPSEEK, "deepseek", "deepseek-flash");
    assert!(matches!(found, Found::Matched(_)));
    let from = json!({"from": "catalog", "entry": "deepseek/deepseek-flash", "layer": 2, "fetched": "2026-10-01T03:25:54.000Z"});
    let with = |value: serde_json::Value| {
        let mut fact = from.clone();
        fact["value"] = value;
        fact
    };
    assert_eq!(
        facts.json(&file),
        json!({
            "window": with(json!(1_000_000)),
            "max_output": with(json!(393_216)),
            "inputs": with(json!(["text", "image"])),
            "tools": with(json!(true)),
            "reasoning": with(json!(["low", "high", "max"])),
            "effort": {"value": null, "from": "default"},
            "price": with(json!({"input": 0.15, "output": 0.6, "cache_read": 0.003, "reasoning": 0.6, "currency": "USD"})),
            "multiplier": {"value": 1.0, "from": "default"},
            "name": with(json!("DeepSeek V4.1 Flash")),
            "status": {"value": null, "from": "default"},
        })
    );
    assert_eq!(
        facts.driver_inputs(),
        Inputs {
            images: true,
            pdf: false
        }
    );
}

/// 每一格各查各的：手写了窗口的，最大输出照样从目录借；手写的来源写文件和行。
#[test]
fn each_fact_is_looked_up_on_its_own() {
    let source = "[providers.deepseek]\nkeys = []\n\n[providers.deepseek.models.\"deepseek-flash\"]\nwindow = 60000\ninputs = [\"text\"]\ntools = false\nreasoning = [\"high\"]\n";
    let (facts, _) = facts_of(&held(), source, "deepseek", "deepseek-flash");
    assert_eq!(facts.window.value, Some(60_000));
    assert_eq!(
        facts.window.source.json(&file),
        json!({"from": "config", "file": "system/config.toml", "line": 5})
            .as_object()
            .cloned()
            .expect("对象")
    );
    assert_eq!(facts.max_output.value, Some(393_216));
    assert!(matches!(facts.max_output.source, Source::Catalog { .. }));
    assert_eq!(facts.inputs.value, ["text"]);
    assert_eq!(facts.tools.value, Some(false));
    assert_eq!(facts.reasoning.value, Some(vec!["high".to_string()]));
    assert!(matches!(facts.price.source, Source::Catalog { .. }));
}

/// 思考强度（施工 8-18）：目录的开关只在档案写了开关时算，多一档 `off`；手写的几档规整过、盖过目录的；默认的那一档只认
/// 写在档位里的，`none` 读成 `off`，来源写文件和行；不在档位里的照没写。
#[test]
fn reasoning_levels_follow_the_profile_and_the_default_must_be_one_of_them() {
    let toggled = Held::new(
        json!({
            "npm": {"@ai-sdk/openai-compatible": "openai-chat"},
            "providers": {"deepseek": {"compat": {"toggle": {"field": "thinking", "on": true, "off": false}}}}
        }),
        true,
    );
    let (facts, _) = facts_of(&toggled, DEEPSEEK, "deepseek", "deepseek-flash");
    assert_eq!(
        facts.reasoning.value,
        Some(["off", "low", "high", "max"].map(str::to_string).to_vec()),
        "档案写了开关：多一档 off"
    );
    let (plain, _) = facts_of(&held(), DEEPSEEK, "deepseek", "deepseek-flash");
    assert_eq!(plain.levels(), ["low", "high", "max"], "档案没写开关：不加");
    let written = "[providers.deepseek]\nkeys = []\n\n[providers.deepseek.models.\"deepseek-flash\"]\neffort = \"none\"\n";
    let (facts, _) = facts_of(&toggled, written, "deepseek", "deepseek-flash");
    assert_eq!(facts.effort.value.as_deref(), Some("off"));
    assert_eq!(
        Json::Object(facts.effort.source.json(&file)),
        json!({"from": "config", "file": "system/config.toml", "line": 5})
    );
    let (facts, _) = facts_of(&held(), written, "deepseek", "deepseek-flash");
    assert_eq!(
        (facts.effort.value, facts.effort.source),
        (None, Source::Default),
        "没有开关就没有 off：照没写"
    );
    let own = "[providers.deepseek]\nkeys = []\n\n[providers.deepseek.models.\"deepseek-flash\"]\nreasoning = [\"disabled\", \"turbo\"]\neffort = \"turbo\"\n";
    let (facts, _) = facts_of(&toggled, own, "deepseek", "deepseek-flash");
    assert_eq!(facts.levels(), ["off", "turbo"], "手写的盖过目录，照样规整");
    assert_eq!(facts.effort.value.as_deref(), Some("turbo"));
}

/// 窗口：手写的、用出来的、供应商的列表、目录，先有的算。
#[test]
fn the_window_goes_written_learned_listed_catalog() {
    let mut held = held();
    let learned_at = at("2026-10-01T08:12:30.000Z");
    let fetched = at("2026-10-01T03:00:00.000Z");
    held.lists.insert(
        "deepseek".to_string(),
        ProviderList {
            fetched,
            models: vec![ListedModel {
                id: "deepseek-flash".to_string(),
                window: Some(500_000),
            }],
        },
    );
    let (facts, _) = facts_of(&held, DEEPSEEK, "deepseek", "deepseek-flash");
    assert_eq!(facts.window.value, Some(500_000));
    assert_eq!(facts.window.source, Source::Provider { fetched });
    held.learned
        .learn("deepseek", "deepseek-flash", 65_536, learned_at);
    let (facts, _) = facts_of(&held, DEEPSEEK, "deepseek", "deepseek-flash");
    assert_eq!(facts.window.value, Some(65_536));
    assert_eq!(facts.window.source, Source::Learned { at: learned_at });
    assert_eq!(
        facts.window.source.json(&file),
        json!({"from": "learned", "at": "2026-10-01T08:12:30.000Z"})
            .as_object()
            .cloned()
            .expect("对象")
    );
    let written =
        format!("{DEEPSEEK}\n[providers.deepseek.models.\"deepseek-flash\"]\nwindow = 70000\n");
    let (facts, _) = facts_of(&held, &written, "deepseek", "deepseek-flash");
    assert_eq!(facts.window.value, Some(70_000), "手写的盖过用出来的");
    // 列表、用出来的只管这一家：别家同名的不算。
    let (facts, _) = facts_of(
        &held,
        "[providers.other]\ndriver = \"openai-chat\"\nbase_url = \"https://x.invalid\"\n",
        "other",
        "deepseek-flash",
    );
    assert!(matches!(
        facts.window.source,
        Source::Catalog { layer: 3, .. }
    ));
}

/// 价格是一整格：手写了一项就整份用手写的，币种不写是美元，写了照写的。
#[test]
fn a_written_price_is_taken_whole() {
    let source = format!(
        "{DEEPSEEK}\n[providers.deepseek.models.\"deepseek-flash\".price]\ninput = 1\ncurrency = \"CNY\"\n"
    );
    let (facts, _) = facts_of(&held(), &source, "deepseek", "deepseek-flash");
    let price = facts.price.value.expect("有价");
    assert_eq!(
        price.json(),
        json!({"input": 1.0, "currency": "CNY"}),
        "输出价不从目录拼"
    );
    assert!(
        matches!(facts.price.source, Source::Config { line: 5, .. }),
        "第一项写在哪一行"
    );
    let source = format!(
        "{DEEPSEEK}\n[providers.deepseek.models.\"deepseek-flash\"]\nprice = {{ output = 2.5 }}\n"
    );
    let (facts, _) = facts_of(&held(), &source, "deepseek", "deepseek-flash");
    assert_eq!(
        facts.price.value.map(|price| price.json()),
        Some(json!({"output": 2.5, "currency": "USD"}))
    );
}

/// 第 3、4 层：能力、窗口、名字照借，价格照挑的那家借不借。
#[test]
fn name_matches_borrow_all_but_an_unofficial_price() {
    let relay = "[providers.newapi]\ndriver = \"openai-chat\"\nbase_url = \"https://relay.example.invalid/v1\"\n";
    let (facts, _) = facts_of(&held(), relay, "newapi", "deepseek-v4.1-flash");
    let from = |source: &Source| match source {
        Source::Catalog { entry, layer, .. } => Some((entry.clone(), *layer)),
        _ => None,
    };
    let above = Some(("above/deepseek-v4.1-flash".to_string(), 3));
    assert_eq!(facts.window.value, Some(1_000_000));
    assert_eq!(from(&facts.window.source), above);
    assert_eq!(from(&facts.tools.source), above);
    assert_eq!(facts.price.value, None, "照字节序挑的，不借价");
    assert_eq!(facts.price.source, Source::Default);
    let (facts, _) = facts_of(&held(), relay, "newapi", "claude-sonnet-4-5");
    assert_eq!(
        facts.price.value.and_then(|price| price.rates.input),
        Some(3.0),
        "原厂的价"
    );
}

/// 本机的服务：价格只认手写的，没写是 0，来源 `local`；倍率照样算。
#[test]
fn a_local_service_is_free_unless_priced_by_hand() {
    let local =
        "[providers.ollama]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1:11434/v1\"\n";
    let (facts, _) = facts_of(&held(), local, "ollama", "deepseek-v4.1-flash");
    assert_eq!(facts.price.value, Some(Price::free()));
    assert_eq!(facts.price.source, Source::Local);
    assert_eq!(facts.window.value, Some(1_000_000), "能力、窗口照借");
    let priced =
        format!("{local}\n[providers.ollama.models.\"deepseek-v4.1-flash\".price]\ninput = 0.5\n");
    let (facts, _) = facts_of(&held(), &priced, "ollama", "deepseek-v4.1-flash");
    assert_eq!(
        facts.price.value.and_then(|price| price.rates.input),
        Some(0.5)
    );
}

/// 倍率：模型的，再是供应商的，都没有是 1。
#[test]
fn the_model_multiplier_beats_the_provider_one() {
    let source = "[providers.deepseek]\nprice_multiplier = 0.5\n\n[providers.deepseek.models.\"deepseek-v4-pro\"]\nprice_multiplier = 2\n";
    let (flash, _) = facts_of(&held(), source, "deepseek", "deepseek-flash");
    assert_eq!(flash.multiplier.value, 0.5);
    assert!(matches!(
        flash.multiplier.source,
        Source::Config { line: 2, .. }
    ));
    let (pro, _) = facts_of(&held(), source, "deepseek", "deepseek-v4-pro");
    assert_eq!(pro.multiplier.value, 2.0);
    assert!(matches!(
        pro.multiplier.source,
        Source::Config { line: 5, .. }
    ));
}

/// 手写指定的条目不存在：不借目录，交回标出来的那一条；没有目录的什么都不借。
#[test]
fn a_missing_hand_pick_borrows_nothing() {
    let source = format!(
        "{DEEPSEEK}\n[providers.deepseek.models.\"deepseek-flash\"]\ncatalog = \"deepseek/nope\"\n"
    );
    let (facts, found) = facts_of(&held(), &source, "deepseek", "deepseek-flash");
    assert_eq!(found, Found::Missing("deepseek/nope".to_string()));
    assert_eq!(facts.window.value, None);
    assert_eq!(facts.name.value, "deepseek-flash");
    assert_eq!(facts.name.source, Source::Default);
    assert_eq!(facts.inputs.value, ["text"], "驱动的保守默认：只有文字");
    let mut empty = held();
    empty.catalog = None;
    let resolved = resolved(DEEPSEEK);
    let deepseek = crate::provider::Provider {
        id: "deepseek".to_string(),
        driver: crate::provider::Driver::OpenAiChat,
        base_url: miyu_config::Address::Literal("https://api.deepseek.com".to_string()),
        compat: miyu_drivers::openai_chat::Compat::default(),
        keys: Vec::new(),
        images: None,
        catalog: "deepseek".to_string(),
        recognized: None,
        local: false,
    };
    let (facts, found) = super::facts(&resolved, &empty.knowledge(), &deepseek, "deepseek-flash");
    assert_eq!(found, Found::Nothing);
    assert_eq!(facts.window.source, Source::Default);
}

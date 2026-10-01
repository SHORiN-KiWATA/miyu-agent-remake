//! 第一次接入的纯逻辑（施工 8-11）：每一家合起来、找哪些变量、探哪几家、搜和排、推荐、候选写成的最终值。

use serde_json::json;

use super::*;
use crate::provider::{Driver, provider};
use crate::test_support::{Held, resolved};

/// 出厂的 `[npm]`、DeepSeek、Ollama，加几家本机、外面的，带上裁出来的目录。
fn held() -> Held {
    Held::new(
        json!({
            "npm": {
                "@ai-sdk/openai-compatible": "openai-chat",
                "@ai-sdk/anthropic": "anthropic",
                "@ai-sdk/openai": "openai-responses"
            },
            "providers": {
                "deepseek": {"driver": "openai-chat", "base_url": "https://api.deepseek.com"},
                "ollama": {"name": "Ollama", "driver": "openai-chat", "base_url": "http://127.0.0.1:11434/v1"},
                "lab": {"driver": "openai-chat", "base_url": "http://LOCALHOST:8080/v1"},
                "six": {"driver": "openai-chat", "base_url": "http://[::1]:9/v1"},
                "remote": {"driver": "openai-chat", "base_url": "https://remote.invalid/v1"},
                "claude-here": {"driver": "anthropic", "base_url": "http://127.0.0.1:7/v1"}
            }
        }),
        true,
    )
}

fn find<'a>(listed: &'a [Listed], id: &str) -> &'a Listed {
    listed
        .iter()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("没有 {id}"))
}

#[test]
fn every_provider_of_the_catalog_and_the_profiles_is_listed_once() {
    let held = held();
    let listed = listed(&held.knowledge());
    let ids: Vec<&str> = listed.iter().map(|entry| entry.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(ids, sorted, "照编号排、不重");
    assert_eq!(
        find(&listed, "ollama"),
        &Listed {
            id: "ollama".to_string(),
            name: "Ollama".to_string(),
            driver: Some("openai-chat".to_string()),
            base_url: Some("http://127.0.0.1:11434/v1".to_string()),
            env: Vec::new(),
            doc: None,
            models: 0,
            supported: true,
            local: true,
        },
        "只在档案里的一家也列"
    );
    let deepseek = find(&listed, "deepseek");
    assert_eq!(deepseek.name, "DeepSeek", "名字照目录");
    assert_eq!(deepseek.env, ["DEEPSEEK_API_KEY"]);
    assert_eq!(deepseek.models, 4, "坏的模型不算");
    assert!(deepseek.supported && !deepseek.local);
    let anthropic = find(&listed, "anthropic");
    assert_eq!(anthropic.driver.as_deref(), Some("anthropic"));
    assert_eq!(anthropic.base_url, None, "目录里没有地址");
    assert!(!anthropic.supported);
    let deepinfra = find(&listed, "deepinfra");
    assert_eq!(deepinfra.driver, None, "包名认不出");
    assert!(!deepinfra.supported);
    let aihubmix = find(&listed, "aihubmix");
    assert_eq!(aihubmix.name, "AIHubMix");
    assert!(!find(&listed, "claude-here").supported, "驱动还没有");
}

#[test]
fn the_json_has_every_field() {
    let held = held();
    let listed = listed(&held.knowledge());
    assert_eq!(
        find(&listed, "anthropic").json(),
        json!({"id": "anthropic", "name": "Anthropic", "driver": "anthropic", "base_url": null,
            "env": ["ANTHROPIC_API_KEY"], "doc": "https://docs.anthropic.com/en/docs/about-claude/models",
            "models": 1, "supported": false, "local": false})
    );
}

#[test]
fn only_a_single_name_is_a_key_and_each_provider_gets_its_line() {
    let held = Held::new(json!({}), true);
    let listed = listed(&held.knowledge());
    let vars = key_vars(&listed);
    let opencode: Vec<&str> = vars
        .iter()
        .filter(|(env, _)| env == "OPENCODE_API_KEY")
        .map(|(_, entry)| entry.id.as_str())
        .collect();
    assert_eq!(
        opencode,
        ["opencode-go", "opencode"],
        "照名字排：OpenCode Go 在前"
    );
    let names = looked_for(&listed);
    assert_eq!(
        names
            .iter()
            .filter(|name| *name == "OPENCODE_API_KEY")
            .count(),
        1
    );
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
    assert!(names.contains(&"DEEPSEEK_API_KEY".to_string()));
}

#[test]
fn several_names_are_not_looked_for() {
    let mut held = Held::new(json!({}), false);
    let text = r#"{"azure":{"name":"Azure","env":["AZURE_RESOURCE_NAME","AZURE_API_KEY"],"npm":"@ai-sdk/azure","models":{}},
        "one":{"name":"One","env":["ONE_KEY"],"models":{}},"none":{"name":"None","env":[],"models":{}}}"#;
    held.catalog = Some(crate::catalog::Loaded {
        catalog: crate::catalog::Catalog::parse(text)
            .expect("读得进")
            .catalog,
        source: crate::catalog::CatalogSource::Snapshot,
        fetched: "2026-10-01T00:00:00.000Z".to_string(),
    });
    let listed = listed(&held.knowledge());
    assert_eq!(looked_for(&listed), ["ONE_KEY"]);
    assert_eq!(key_vars(&listed).len(), 1);
}

#[test]
fn only_usable_providers_on_this_machine_are_probed() {
    let held = held();
    let listed = listed(&held.knowledge());
    let local: Vec<&str> = local_services(&listed)
        .into_iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(local, ["lab", "ollama", "six"], "外面的、驱动还没有的不探");
}

#[test]
fn search_looks_in_ids_and_names_and_puts_usable_ones_first() {
    let held = held();
    let listed = listed(&held.knowledge());
    let ids = |query: Option<&str>, limit: usize| -> Vec<String> {
        search(&listed, query, limit)
            .into_iter()
            .map(|entry| entry.id.clone())
            .collect()
    };
    assert_eq!(
        ids(Some("DEEP"), 50),
        ["deepseek", "deepinfra"],
        "编号、名字里有的都算，不分大小写；能用的在前"
    );
    assert_eq!(ids(Some("hubmix"), 50), ["aihubmix"], "名字里的一截");
    assert_eq!(ids(Some("nothing-like-it"), 50), Vec::<String>::new());
    let all = ids(None, 500);
    assert_eq!(all.len(), listed.len());
    assert_eq!(ids(Some(""), 500), all, "空的等于没写");
    let usable = listed.iter().filter(|entry| entry.supported).count();
    let names: Vec<String> = all
        .iter()
        .map(|id| find(&listed, id).name.to_lowercase())
        .collect();
    let (front, back) = names.split_at(usable);
    for half in [front, back] {
        let mut sorted = half.to_vec();
        sorted.sort();
        assert_eq!(half, sorted.as_slice(), "名字不分大小写排");
    }
    assert!(all[..usable].iter().all(|id| find(&listed, id).supported));
    assert_eq!(ids(None, 2).len(), 2, "照 limit 截");
    assert_eq!(&all[..2], ids(None, 2).as_slice());
}

fn offered(
    model: &str,
    tools: Option<bool>,
    window: Option<u64>,
    released: Option<&str>,
) -> Offered {
    Offered {
        model: model.to_string(),
        tools,
        window,
        status: None,
        released: released.map(str::to_string),
    }
}

#[test]
fn the_latest_fit_model_is_recommended() {
    let fit = |model, released| offered(model, Some(true), Some(128_000), released);
    let models = [
        fit("old", Some("2025-01-01")),
        fit("new", Some("2026-09-10")),
        fit("same-day", Some("2026-09-10")),
        offered("no-tools", Some(false), Some(1_000_000), Some("2027-01-01")),
        offered("unknown-tools", None, Some(1_000_000), Some("2027-01-01")),
        offered("small", Some(true), Some(63_999), Some("2027-01-01")),
        offered("no-window", Some(true), None, Some("2027-01-01")),
        Offered {
            status: Some("deprecated".to_string()),
            ..fit("retired", Some("2027-01-01"))
        },
        fit("undated", None),
    ];
    assert_eq!(recommend(&models), Some("new"), "一样新的取靠前的");
    assert_eq!(
        recommend(&[fit("undated", None), fit("dated", Some("2020-01-01"))]),
        Some("dated"),
        "没有日期的排后面"
    );
    assert_eq!(
        recommend(&[fit("first", None), fit("second", None)]),
        Some("first")
    );
    assert_eq!(
        recommend(&[
            offered("tiny", Some(true), Some(8_000), Some("2027-01-01")),
            offered("exactly", Some(true), Some(64_000), None),
        ]),
        Some("exactly"),
        "64000 够格，不是因为它排第一"
    );
    assert_eq!(
        recommend(&models[3..8]),
        Some("no-tools"),
        "一个都不够格的取第一个"
    );
    assert_eq!(recommend(&[]), None);
}

#[test]
fn a_candidate_is_worked_out_like_the_config_it_would_write() {
    let held = held();
    let candidate = Candidate {
        driver: None,
        base_url: None,
        key: Some(KeyRef::Env("DEEPSEEK_API_KEY".to_string())),
        catalog: Some("deepseek".to_string()),
    };
    assert_eq!(candidate.id(), "deepseek");
    let tried = provider(&candidate.values(), &held.knowledge(), "deepseek").expect("推得出");
    let written =
        resolved("[providers.deepseek]\nkeys = [{ env = \"DEEPSEEK_API_KEY\" }]\n").values();
    let real = provider(&written, &held.knowledge(), "deepseek").expect("推得出");
    assert_eq!(tried, real, "试的和写进配置以后真用的一样");

    let local = Candidate {
        catalog: Some("ollama".to_string()),
        ..Candidate::default()
    };
    let ollama = provider(&local.values(), &held.knowledge(), "ollama").expect("档案推得出");
    assert!(ollama.keys.is_empty() && ollama.local);
    let written = resolved("[providers.ollama]\nkeys = []\n").values();
    assert_eq!(
        Some(ollama),
        provider(&written, &held.knowledge(), "ollama").ok()
    );

    let custom = Candidate {
        driver: Some("openai-chat".to_string()),
        base_url: Some("https://relay.invalid/v1".to_string()),
        key: Some(value_key()),
        catalog: None,
    };
    assert_eq!(custom.id(), CANDIDATE);
    let relay = provider(&custom.values(), &held.knowledge(), CANDIDATE).expect("手写的");
    assert_eq!(relay.driver, Driver::OpenAiChat);
    assert_eq!(relay.keys, [value_key()]);
    let bare = Candidate::default();
    let problem = provider(&bare.values(), &held.knowledge(), CANDIDATE).expect_err("推不出");
    assert!(
        problem.0.contains("needs driver and base_url"),
        "{problem:?}"
    );
}

#[test]
fn the_value_key_never_names_a_real_secret() {
    match value_key() {
        KeyRef::Secret(name) => assert!(!miyu_config::secret::valid_name(&name), "{name}"),
        other => panic!("{other:?}"),
    }
}

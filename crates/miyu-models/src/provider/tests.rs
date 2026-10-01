//! 一家供应商这一轮的样子、一个引用发给谁（施工 8-6）：手写的压过档案、档案照 `catalog` 找、推不出来的说清楚、没配
//! `models.chat` 的是 `no_model` 的那一句。照目录推驱动、地址，本机的服务（施工 8-7）。

use serde_json::json;

use super::*;
use crate::matching::How;
use crate::test_support::{Held, resolved};

fn values(source: &str) -> Values {
    resolved(source).values()
}

/// DeepSeek 的档案，`[npm]` 认 OpenAI 兼容的包；`catalog` 为真的带上裁出来的目录。
fn held(catalog: bool) -> Held {
    Held::new(
        json!({
            "npm": {"@ai-sdk/openai-compatible": "openai-chat", "@ai-sdk/anthropic": "anthropic"},
            "providers": {"deepseek": {
                "driver": "openai-chat",
                "base_url": "https://api.deepseek.com",
                "image_tokens": "deepseek",
                "compat": {"reasoning": {"replay": "reasoning_content", "always": true}}
            }}
        }),
        catalog,
    )
}

#[test]
fn a_known_provider_needs_only_its_keys() {
    let values = values("[providers.deepseek]\nkeys = [{ env = \"DEEPSEEK_API_KEY\" }]\n");
    let held = held(false);
    let deepseek = provider(&values, &held.knowledge(), "deepseek").expect("档案推得出");
    assert_eq!(deepseek.driver, Driver::OpenAiChat);
    assert_eq!(
        deepseek.base_url,
        Address::Literal("https://api.deepseek.com".to_string())
    );
    assert_eq!(deepseek.keys, [KeyRef::Env("DEEPSEEK_API_KEY".to_string())]);
    assert_eq!(deepseek.images, Some(ImageTokens::DeepSeek));
    assert_ne!(deepseek.compat, Compat::default(), "开关照档案");
    assert_eq!(deepseek.catalog, "deepseek");
    assert_eq!(deepseek.recognized, None, "还没有目录");
    assert!(!deepseek.local);
}

#[test]
fn hand_written_values_win_and_the_profile_is_found_by_catalog() {
    let values = values(
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"https://relay.invalid/v1\"\ncatalog = \"deepseek\"\n\n[providers.plain]\ndriver = \"openai-chat\"\nbase_url = \"http://plain.invalid:9/v1\"\n",
    );
    let held = held(false);
    let dev = provider(&values, &held.knowledge(), "dev").expect("手写的");
    assert_eq!(
        dev.base_url,
        Address::Literal("https://relay.invalid/v1".to_string()),
        "手写的压过档案"
    );
    assert_eq!(
        Some(dev.compat),
        held.profiles.providers["deepseek"]
            .compat
            .as_ref()
            .map(|c| c.compat())
    );
    assert_eq!(dev.catalog, "deepseek");
    assert!(dev.keys.is_empty(), "没写 key 的不带认证头");
    let plain = provider(&values, &held.knowledge(), "plain").expect("手写的");
    assert_eq!(
        (plain.compat, plain.images),
        (Compat::default(), None),
        "没有档案的照驱动的默认"
    );
}

#[test]
fn a_provider_that_cannot_be_worked_out_says_why() {
    let values = values(
        "[providers.newapi]\nkeys = []\n\n[providers.claude]\ndriver = \"anthropic\"\nbase_url = \"https://a.invalid\"\n",
    );
    let held = held(true);
    assert_eq!(
        provider(&values, &held.knowledge(), "newapi"),
        Err(NoModel(
            r#"provider "newapi" needs driver and base_url: it matches nothing in the catalog"#
                .to_string()
        ))
    );
    assert_eq!(
        provider(&values, &held.knowledge(), "claude"),
        Err(NoModel(
            r#"driver "anthropic" of provider "claude" is not available yet"#.to_string()
        ))
    );
    assert_eq!(
        provider(&values, &held.knowledge(), "deepseek"),
        Err(NoModel(r#"no provider "deepseek""#.to_string())),
        "档案认得也要配了才算"
    );
}

/// 档案没有的照目录推（施工 8-7）：认出的那一家的 `api`、`npm` 照 `[npm]` 换成的驱动。认不出、`npm` 认不得的推不出。
#[test]
fn the_catalog_fills_in_what_the_profile_lacks() {
    let values = values(
        "[providers.opencodego]\nkeys = []\n\n[providers.anthropic]\nkeys = []\n\n[providers.aihubmix]\nkeys = []\n",
    );
    let held = held(true);
    let go = provider(&values, &held.knowledge(), "opencodego").expect("目录推得出");
    assert_eq!(go.driver, Driver::OpenAiChat);
    assert_eq!(
        go.base_url,
        Address::Literal("https://opencode.ai/zen/go/v1".to_string())
    );
    assert_eq!(
        go.recognized,
        Some(Recognized {
            provider: "opencode-go".to_string(),
            how: How::SimilarId
        })
    );
    assert_eq!(go.catalog, "opencodego", "档案照编号找，不照认出的");
    // Anthropic 认得出，目录里没写地址：推不出。
    assert!(provider(&values, &held.knowledge(), "anthropic").is_err());
    // aihubmix 的包档案里没有：推不出驱动。
    assert!(provider(&values, &held.knowledge(), "aihubmix").is_err());
    // 没有目录的时候照旧推不出。
    assert!(provider(&values, &self::held(false).knowledge(), "opencodego").is_err());
}

/// 本机的服务（施工 8-7）：地址在本机的是，手写的 `local` 盖过地址。
#[test]
fn a_local_service_is_told_by_its_address_or_by_hand() {
    let values = values(
        "[providers.ollama]\ndriver = \"openai-chat\"\nbase_url = \"http://LOCALHOST:11434/v1\"\n\n[providers.lm]\ndriver = \"openai-chat\"\nbase_url = \"http://[::1]:1234/v1\"\n\n[providers.lan]\ndriver = \"openai-chat\"\nbase_url = \"http://192.168.1.9:8080/v1\"\nlocal = true\n\n[providers.loop]\ndriver = \"openai-chat\"\nbase_url = \"http://127.0.0.1/v1\"\nlocal = false\n\n[providers.far]\ndriver = \"openai-chat\"\nbase_url = \"https://localhost.example.invalid/v1\"\n",
    );
    let held = held(false);
    let local = |id: &str| {
        provider(&values, &held.knowledge(), id)
            .expect("配了")
            .local
    };
    assert!(local("ollama"));
    assert!(local("lm"));
    assert!(local("lan"), "手写的");
    assert!(!local("loop"), "手写的盖过地址");
    assert!(!local("far"));
}

#[test]
fn a_reference_resolves_to_a_provider_and_a_model() {
    let values = values(
        "[providers.deepseek]\nkeys = [{ secret = \"deepseek\" }]\n\n[models]\nchat = \"deepseek/deepseek-flash\"\n",
    );
    let held = held(false);
    let knowledge = held.knowledge();
    let found = target(&values, &knowledge, "deepseek/deepseek-flash").expect("解析得出");
    assert_eq!(
        (found.provider.id.as_str(), found.model.as_str()),
        ("deepseek", "deepseek-flash")
    );
    let tier = target(&values, &knowledge, "flagship").expect("挡位没配退回 chat");
    assert_eq!(tier, found);
    assert_eq!(
        target(&values, &knowledge, "@free"),
        Err(NoModel(r#"no pool "free""#.to_string()))
    );
    assert_eq!(
        target(&values, &knowledge, "openai/gpt-5"),
        Err(NoModel(r#"no provider "openai""#.to_string()))
    );
    assert_eq!(
        target(&values, &knowledge, "nope"),
        Err(NoModel(
            r#""nope" is not a model, a pool or a tier"#.to_string()
        ))
    );
    assert_eq!(chat(&values).as_deref(), Some("deepseek/deepseek-flash"));
    let empty = Values::default();
    assert_eq!(chat(&empty), None);
    assert_eq!(
        target(&empty, &knowledge, "lite"),
        Err(NoModel(NOT_CONFIGURED.to_string()))
    );
    assert_eq!(NOT_CONFIGURED, "no model configured: set models.chat");
}

/// 地址是环境变量的引用（施工 8-6b）：对目录认不出（手写的地址查不到字面），本机的服务也查不出来——想算本机的要自己写
/// `local = true`。
#[test]
fn an_address_from_the_environment_skips_recognition_and_local_detection() {
    let values = values(
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = { env = \"RELAY_URL\" }\n\n[providers.loop]\ndriver = \"openai-chat\"\nbase_url = { env = \"LOOP_URL\" }\nlocal = true\n",
    );
    let held = held(true);
    let dev = provider(&values, &held.knowledge(), "dev").expect("配了");
    assert_eq!(dev.base_url, Address::Env("RELAY_URL".to_string()));
    assert_eq!(dev.recognized, None, "字面地址查不到，对不上目录");
    assert!(!dev.local, "查不出来，照不在本机算");
    let looped = provider(&values, &held.knowledge(), "loop").expect("配了");
    assert_eq!(looped.base_url, Address::Env("LOOP_URL".to_string()));
    assert!(looped.local, "手写的 local 照样管用");
}

/// 照引用取地址（施工 8-6b）：写死的直接用；是环境变量的照 `secret` 取，取不到（没设、设成空的）是 `NoModel`，地址不会
/// 出现在错误原话里。
#[test]
fn resolving_the_address_follows_the_reference_or_fails_cleanly() {
    let literal = Provider {
        id: "a".to_string(),
        driver: Driver::OpenAiChat,
        base_url: Address::Literal("https://a.invalid".to_string()),
        compat: Compat::default(),
        keys: Vec::new(),
        images: None,
        catalog: "a".to_string(),
        recognized: None,
        local: false,
    };
    assert_eq!(
        resolve_base_url(&literal, &|_| None),
        Ok("https://a.invalid".to_string())
    );
    let env = Provider {
        base_url: Address::Env("RELAY_URL".to_string()),
        ..literal.clone()
    };
    let set = |reference: &KeyRef| match reference {
        KeyRef::Env(name) if name == "RELAY_URL" => {
            Some(miyu_config::secret::Secret::new("https://relay.invalid").expect("合写法"))
        }
        _ => None,
    };
    assert_eq!(
        resolve_base_url(&env, &set),
        Ok("https://relay.invalid".to_string())
    );
    let unset = resolve_base_url(&env, &|_| None);
    assert_eq!(
        unset,
        Err(NoModel(
            r#"provider "a" has no usable base_url"#.to_string()
        ))
    );
}

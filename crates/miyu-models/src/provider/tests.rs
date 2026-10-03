//! 一家供应商这一轮的样子（施工 8-6）：手写的压过档案、档案照 `catalog` 找、推不出来的说清楚。照目录推驱动、地址，本机的
//! 服务（施工 8-7）。一个引用发给谁挪到 `reference/tests.rs`（施工 8-8）。

use serde_json::json;

use super::*;
use crate::facts::Wire;
use crate::matching::How;
use crate::test_support::{Held, resolved};
use miyu_drivers::openai_chat::{ReasoningField, ReasoningReplay};

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
            },
            // 档案里写了还没有的驱动（施工 8-13：配置里只能写有的三种，还没有的只会从档案、目录来）。
            "gemini": {"driver": "google", "base_url": "https://g.invalid"}}
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
    let values = values("[providers.newapi]\nkeys = []\n\n[providers.gemini]\nkeys = []\n");
    let held = held(true);
    assert_eq!(
        provider(&values, &held.knowledge(), "newapi"),
        Err(NoModel(
            r#"provider "newapi" needs driver and base_url: it matches nothing in the catalog"#
                .to_string()
        ))
    );
    assert_eq!(
        provider(&values, &held.knowledge(), "gemini"),
        Err(NoModel(
            r#"driver "google" of provider "gemini" is not available yet"#.to_string()
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
        driver_written: false,
        reasoning_written: false,
        headers: std::collections::BTreeMap::new(),
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

/// 施工 8-12、8-13：`anthropic`、`openai-responses` 认得了；能不能关思考 openai-chat 照档案、anthropic 自带、openai-responses
/// 没有；一定要写输出上限的只有 anthropic。
#[test]
fn anthropic_and_responses_are_drivers_now() {
    assert_eq!(Driver::parse("anthropic"), Some(Driver::Anthropic));
    assert_eq!(
        Driver::parse("openai-responses"),
        Some(Driver::OpenAiResponses)
    );
    assert_eq!(Driver::parse("google"), None);
    assert!(!Driver::OpenAiResponses.needs_max_output());
    for driver in [
        Driver::OpenAiChat,
        Driver::Anthropic,
        Driver::OpenAiResponses,
    ] {
        assert_eq!(Driver::parse(driver.as_str()), Some(driver));
    }
    assert!(Driver::Anthropic.needs_max_output());
    assert!(!Driver::OpenAiChat.needs_max_output());
    let values = values(
        "[providers.claude]\ndriver = \"anthropic\"\nbase_url = \"https://api.anthropic.com/v1\"\nkeys = []\n\n[providers.deepseek]\nkeys = []\n",
    );
    let held = held(false);
    let claude = provider(&values, &held.knowledge(), "claude").expect("认得了");
    assert_eq!(claude.driver, Driver::Anthropic);
    assert!(claude.switchable(), "开关是接口自带的");
    assert_eq!(claude.build(texts()).family(), "anthropic");
    let deepseek = provider(&values, &held.knowledge(), "deepseek").expect("档案推得出");
    assert!(!deepseek.switchable(), "这份档案没写开关");
    assert_eq!(deepseek.build(texts()).family(), "openai-chat");
    let mut toggled = deepseek.clone();
    toggled.compat = Compat::deepseek();
    assert!(toggled.switchable());
    let mut gpt = toggled.clone();
    gpt.driver = Driver::OpenAiResponses;
    assert!(!gpt.switchable(), "这一家没有开关，档案写了也不算");
    assert_eq!(gpt.build(texts()).family(), "openai-responses");
}

/// 出厂的占位：测试里只要造得出驱动。
fn texts() -> miyu_drivers::DriverTexts {
    miyu_drivers::DriverTexts::new(miyu_drivers::DriverTextSources {
        image_omitted: "x",
        file_omitted: "x",
        no_output: "x",
        tool_attachments: "x",
        tool_attachments_only: "x",
        text_file: None,
        image_name: None,
        image_description: None,
    })
    .expect("造得出")
}

/// 档案：opencode Go 的头、`[npm]` 的三种驱动外加一个还没有的（施工 8-14）；`catalog` 为真的带上裁出来的目录。
fn go_held(catalog: bool) -> Held {
    Held::new(
        json!({
            "npm": {"@ai-sdk/openai-compatible": "openai-chat", "@ai-sdk/anthropic": "anthropic",
                    "@ai-sdk/openai": "openai-responses", "@ai-sdk/google": "google"},
            "providers": {
                "opencode-go": {"headers": {"x-opencode-session": "ses_{session_digest}"}},
                "deepseek": {"compat": {"reasoning": "drop"}}
            }
        }),
        catalog,
    )
}

fn wire(npm: Option<&str>, interleaved: Option<&str>) -> Wire {
    Wire {
        npm: npm.map(str::to_string),
        interleaved: interleaved.map(str::to_string),
    }
}

/// 同一家的模型照目录走三种驱动；模型没写包名的照这一家的；手写的供应商驱动压过模型的包名（施工 8-14）。
#[test]
fn each_model_speaks_through_its_own_driver() {
    let held = go_held(true);
    let npm = &held.profiles.npm;
    let values = values(
        "[providers.opencode-go]\nkeys = []\n\n[providers.fixed]\ndriver = \"openai-chat\"\nbase_url = \"https://opencode.ai/zen/go/v1\"\nkeys = []\n",
    );
    let go = provider(&values, &held.knowledge(), "opencode-go").expect("目录推得出");
    assert_eq!(go.driver, Driver::OpenAiChat, "这一家的包名");
    assert!(!go.driver_written);
    let speak = |provider: &Provider, wire: Wire| {
        provider.for_model("m", &wire, npm).expect("驱动有").driver
    };
    assert_eq!(
        speak(&go, wire(Some("@ai-sdk/anthropic"), None)),
        Driver::Anthropic
    );
    assert_eq!(
        speak(&go, wire(Some("@ai-sdk/openai"), None)),
        Driver::OpenAiResponses
    );
    assert_eq!(speak(&go, wire(None, None)), Driver::OpenAiChat);
    assert_eq!(
        speak(&go, wire(Some("@ai-sdk/openai-compatible"), None)),
        Driver::OpenAiChat
    );
    let fixed = provider(&values, &held.knowledge(), "fixed").expect("手写的");
    assert!(fixed.driver_written);
    assert_eq!(
        speak(&fixed, wire(Some("@ai-sdk/anthropic"), None)),
        Driver::OpenAiChat,
        "手写的供应商驱动压过模型的包名"
    );
    let google = go.for_model("gemini-3-pro", &wire(Some("@ai-sdk/google"), None), npm);
    assert_eq!(
        google,
        Err(NoModel(
            r#"model "opencode-go/gemini-3-pro" needs driver "google", which is not available yet"#
                .to_string()
        ))
    );
    let unknown = go.for_model("x", &wire(Some("@vendor/odd"), None), npm);
    assert_eq!(
        unknown,
        Err(NoModel(
            r#"model "opencode-go/x" needs driver "@vendor/odd", which is not available yet"#
                .to_string()
        )),
        "表里没有的写包名本身"
    );
    let anthropic = go
        .for_model("m", &wire(Some("@ai-sdk/anthropic"), None), npm)
        .expect("有");
    assert!(anthropic.switchable(), "能不能关思考照模型的驱动");
    assert!(anthropic.driver.needs_max_output());
    assert_eq!(anthropic.id, go.id, "别的照这一家");
    assert_eq!(anthropic.keys, go.keys);
}

/// 交错思考（施工 8-14）：走 openai-chat 的照目录的字段回传、`always` 是真的；档案写了 `reasoning` 的照档案；认不出的字段、
/// 不走 openai-chat 的不管。
#[test]
fn interleaved_thinking_is_replayed_unless_the_profile_says_otherwise() {
    let held = go_held(false);
    let npm = &held.profiles.npm;
    let written = values(
        "[providers.opencode-go]\ndriver = \"openai-chat\"\nbase_url = \"https://opencode.ai/zen/go/v1\"\nkeys = []\n\n[providers.deepseek]\ndriver = \"openai-chat\"\nbase_url = \"https://api.deepseek.com\"\nkeys = []\n",
    );
    let go = provider(&written, &held.knowledge(), "opencode-go").expect("手写的");
    assert!(!go.reasoning_written);
    let reasoning = |provider: &Provider, wire: Wire| {
        provider
            .for_model("m", &wire, npm)
            .expect("驱动有")
            .compat
            .reasoning
    };
    assert_eq!(
        reasoning(&go, wire(None, Some("reasoning_content"))),
        ReasoningReplay::Replay {
            field: ReasoningField::ReasoningContent,
            always: true
        }
    );
    assert_eq!(
        reasoning(&go, wire(None, Some("reasoning"))),
        ReasoningReplay::Replay {
            field: ReasoningField::Reasoning,
            always: true
        }
    );
    for odd in [Some("reasoning_details"), None] {
        assert_eq!(
            reasoning(&go, wire(None, odd)),
            ReasoningReplay::Drop,
            "{odd:?}"
        );
    }
    let deepseek = provider(&written, &held.knowledge(), "deepseek").expect("手写的");
    assert!(deepseek.reasoning_written);
    assert_eq!(
        reasoning(&deepseek, wire(None, Some("reasoning_content"))),
        ReasoningReplay::Drop,
        "档案写了 drop 的照档案"
    );
    // 驱动手写成 openai-chat：模型包名不管用，交错思考照样回传。
    assert_eq!(
        reasoning(
            &go,
            wire(Some("@ai-sdk/anthropic"), Some("reasoning_content"))
        ),
        ReasoningReplay::Replay {
            field: ReasoningField::ReasoningContent,
            always: true
        }
    );
    let free = provider(
        &values("[providers.opencode-go]\nkeys = []\n"),
        &go_held(true).knowledge(),
        "opencode-go",
    )
    .expect("目录推得出");
    let claude = free
        .for_model(
            "m",
            &wire(Some("@ai-sdk/anthropic"), Some("reasoning_content")),
            npm,
        )
        .expect("有");
    assert_eq!(
        claude.compat.reasoning,
        ReasoningReplay::Drop,
        "不走 openai-chat 的不管"
    );
}

/// 另配的头照档案，值照种子换（施工 8-14）：同一个种子同一个值，没写头的一家什么都不加。
#[test]
fn headers_come_from_the_profile_and_fill_with_the_seed() {
    let held = go_held(true);
    let values = values("[providers.opencode-go]\nkeys = []\n\n[providers.deepseek]\nkeys = []\n");
    let go = provider(&values, &held.knowledge(), "opencode-go").expect("目录推得出");
    assert_eq!(
        go.headers("ses-1"),
        [(
            "x-opencode-session".to_string(),
            "ses_09df36791e54c51f7be063867e".to_string()
        )]
    );
    assert_eq!(go.headers("ses-1"), go.headers("ses-1"));
    assert_ne!(go.headers("ses-1"), go.headers("ses-2"));
    let deepseek = provider(&values, &held.knowledge(), "deepseek").expect("目录推得出");
    assert!(deepseek.headers("ses-1").is_empty());
}

//! 模型（施工 8-6、8-7）：出厂的档案读得进来、DeepSeek 那一套开关和请求形状探针用的那一套一样（档案替了原来写在代码里的
//! `Compat::deepseek()`）；TOML 怎么变成 JSON；出厂的目录快照、认原厂的表读得进来、对得上。

use miyu_drivers::openai_chat::Compat;
use std::collections::BTreeMap;

use miyu_models::Knowledge;
use miyu_models::catalog::{Catalog, CatalogSource, Loaded};
use miyu_models::observed::Learned;
use miyu_models::profile::ImageTokens;

use super::*;

#[test]
fn the_shipped_profiles_keep_what_was_in_the_code() {
    let profiles = profiles(include_str!("../../../../resources/models/profiles.toml"))
        .expect("出厂的档案读得进来");
    let deepseek = &profiles.providers["deepseek"];
    assert_eq!(
        (deepseek.driver.as_deref(), deepseek.base_url.as_deref()),
        (Some("openai-chat"), Some("https://api.deepseek.com"))
    );
    assert_eq!(
        deepseek.compat.as_ref().map(|compat| compat.compat()),
        Some(Compat::deepseek()),
        "请求形状探针照 Compat::deepseek() 编码，出厂的要和它一样"
    );
    assert_eq!(profiles.npm["@ai-sdk/openai-compatible"], "openai-chat");
    assert_eq!(deepseek.image_tokens, Some(ImageTokens::DeepSeek));
    // 施工 8-11：Ollama 只为第一次接入找本机的服务，目录里没有，名字、驱动、地址都在档案里。
    let ollama = &profiles.providers["ollama"];
    assert_eq!(
        (
            ollama.name.as_deref(),
            ollama.driver.as_deref(),
            ollama.base_url.as_deref()
        ),
        (
            Some("Ollama"),
            Some("openai-chat"),
            Some("http://127.0.0.1:11434/v1")
        )
    );
    // 施工 8-12：Anthropic 官方的地址目录里没有，档案补上；没有开关。
    let anthropic = &profiles.providers["anthropic"];
    assert_eq!(
        (anthropic.driver.as_deref(), anthropic.base_url.as_deref()),
        (Some("anthropic"), Some("https://api.anthropic.com/v1"))
    );
    assert!(anthropic.compat.is_none());
    assert_eq!(profiles.npm["@ai-sdk/anthropic"], "anthropic");
    // 施工 8-13：OpenAI 官方一样，目录里没有地址，档案补上。
    let openai = &profiles.providers["openai"];
    assert_eq!(
        (openai.driver.as_deref(), openai.base_url.as_deref()),
        (Some("openai-responses"), Some("https://api.openai.com/v1"))
    );
    assert!(openai.compat.is_none());
    assert_eq!(profiles.npm["@ai-sdk/openai"], "openai-responses");
    // 施工 8-14：opencode Go 只多一个头，驱动、地址照目录。
    let go = &profiles.providers["opencode-go"];
    assert_eq!(
        go.headers.iter().collect::<Vec<_>>(),
        [(
            &"x-opencode-session".to_string(),
            &"ses_{session_digest}".to_string()
        )]
    );
    assert_eq!((go.driver.as_deref(), go.base_url.as_deref()), (None, None));
    assert!(go.compat.is_none());
    // 施工 8-14 补（2026-10-04 实测）：Zen 免费档按客户端识别——User-Agent 盖成 opencode 的形状，另配三个头，
    // 工具面里缺 `read`、`shell` 的补占位；说明是 `resources/core/drivers/placeholder-tool.txt` 那一句。
    let zen = &profiles.providers["opencode"];
    assert_eq!(zen.headers["User-Agent"], "opencode/2.0.21");
    assert_eq!(zen.headers["x-opencode-client"], "cli");
    assert_eq!(zen.headers["x-opencode-project"], "global");
    assert_eq!(zen.headers["x-opencode-session"], "ses_{session_digest}");
    assert_eq!(zen.placeholder_tools, ["read", "shell"]);
    assert!(
        !include_str!("../../../../resources/core/drivers/placeholder-tool.txt").is_empty(),
        "占位说明在资源里"
    );
    for (id, profile) in &profiles.providers {
        if id != "opencode-go" && id != "opencode" {
            assert!(profile.headers.is_empty(), "{id}");
        }
    }
}

#[test]
fn toml_becomes_json_and_bad_profiles_say_so() {
    let read =
        profiles("[providers.a]\ndriver = \"openai-chat\"\ncompat = { stream_usage = false }\n")
            .expect("读得进来");
    assert_eq!(read.providers["a"].driver.as_deref(), Some("openai-chat"));
    for bad in [
        "[providers.a\n",
        "[providers.a]\nwhen = 2026-10-01\n",
        "[providers.a]\nunknown = 1\n",
    ] {
        let error = profiles(bad).expect_err("读不进来");
        assert!(
            error.starts_with("models/profiles.toml not readable: "),
            "{error}"
        );
    }
}

/// 出厂的目录快照读得进来（施工 8-7）：原样的 `api.json`，DeepSeek 官方的 `deepseek-flash` 在里面，收图；`meta` 说得出
/// 来源和时刻。认原厂的表读得进来，每个原厂都是快照里有的编号。
#[test]
fn the_bundled_catalog_and_vendor_table_read() {
    let read = Catalog::parse(include_str!("../../../../resources/models/models-dev.json"))
        .expect("读得出来");
    let catalog = read.catalog;
    assert!(catalog.providers().count() > 100, "完整的目录");
    let flash = catalog.model("deepseek", "deepseek-flash").expect("有");
    assert_eq!(
        (flash.window, flash.max_output),
        (Some(1_000_000), Some(393_216))
    );
    assert!(
        flash
            .inputs
            .as_ref()
            .is_some_and(|inputs| inputs.iter().any(|input| input == "image"))
    );
    let meta: catalog::Meta = serde_json::from_str(include_str!(
        "../../../../resources/models/models-dev.meta.json"
    ))
    .expect("meta 读得出来");
    assert_eq!(meta.source, "https://models.dev/api.json");
    assert!(
        miyu_kernel::time::Timestamp::parse(&meta.fetched).is_ok(),
        "{}",
        meta.fetched
    );
    let text = include_str!("../../../../resources/models/vendors.toml");
    vendors(text).expect("读得进来");
    // 表里写的每一个原厂（引号里的）都是快照里有的编号：写错了这一家就认不出原厂。
    let ids: Vec<&str> = text
        .lines()
        .filter(|line| !line.starts_with('#'))
        .flat_map(|line| line.split('"').skip(1).step_by(2))
        .collect();
    assert!(ids.len() >= 10, "{ids:?}");
    for id in ids {
        assert!(catalog.provider(id).is_some(), "原厂 {id} 在目录里");
    }
    assert!(vendors("gpt = 1\n").is_err());
}

/// 出厂的常用几家（施工 8-11 再补）读得进来，写的每一个编号（`catalog`、`catalog_zh`）照出厂的目录快照、档案都是能用的：写错了
/// 这一家就从 `miyu setup`、第一次引导里不声不响地消失，推不出驱动的标成用不了（OpenRouter 曾这样，档案补了驱动）。
#[test]
fn every_featured_provider_is_usable_with_the_bundled_data() {
    let featured =
        miyu_models::onboard::featured(include_str!("../../../../resources/models/featured.toml"))
            .expect("读得进来");
    let loaded = Loaded {
        catalog: Catalog::parse(include_str!("../../../../resources/models/models-dev.json"))
            .expect("读得出来")
            .catalog,
        source: CatalogSource::Snapshot,
        fetched: String::new(),
    };
    let profiles = profiles(include_str!("../../../../resources/models/profiles.toml"))
        .expect("出厂的档案读得进来");
    let vendors =
        vendors(include_str!("../../../../resources/models/vendors.toml")).expect("读得进来");
    let knowledge = Knowledge {
        profiles: &profiles,
        vendors: &vendors,
        catalog: Some(&loaded),
        learned: &Learned::default(),
        lists: &BTreeMap::new(),
    };
    let listed = miyu_models::onboard::listed(&knowledge);
    assert_eq!(
        featured.len(),
        14,
        "项目主人 2026-10-10 照 opencode 定的，Google 接不上不列（施工 T-12）"
    );
    for one in &featured {
        for id in std::iter::once(&one.catalog).chain(&one.catalog_zh) {
            let entry = listed.iter().find(|entry| &entry.id == id);
            assert!(
                entry.is_some_and(|entry| entry.supported),
                "{id} 用不了：{entry:?}"
            );
        }
        assert!(one.name.pick("zh").is_some() && one.name.pick("en").is_some());
    }
}

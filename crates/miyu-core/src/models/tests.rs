//! 模型（施工 8-6、8-7）：出厂的档案读得进来、DeepSeek 那一套开关和请求形状探针用的那一套一样（档案替了原来写在代码里的
//! `Compat::deepseek()`）；TOML 怎么变成 JSON；出厂的目录快照、认原厂的表读得进来、对得上。

use miyu_drivers::openai_chat::Compat;
use miyu_models::catalog::Catalog;
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

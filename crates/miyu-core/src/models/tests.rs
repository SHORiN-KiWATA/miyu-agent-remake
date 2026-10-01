//! 模型（施工 8-6）：出厂的档案读得进来、DeepSeek 那一套开关和请求形状探针用的那一套一样（档案替了原来写在代码里的
//! `Compat::deepseek()`）；TOML 怎么变成 JSON。

use miyu_drivers::openai_chat::Compat;
use miyu_models::profile::ImageTokens;
use miyu_models::{ModelFacts, ModelTable};

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
    let inputs = deepseek.inputs();
    assert!(
        inputs.images && !inputs.pdf,
        "DeepSeek 收图，不收 PDF（施工 4-13）"
    );
    assert_eq!(deepseek.image_tokens, Some(ImageTokens::DeepSeek));
}

#[test]
fn toml_becomes_json_and_bad_profiles_say_so() {
    let read = profiles(
        "[providers.a]\ndriver = \"openai-chat\"\ninputs = [\"text\"]\ncompat = { stream_usage = false }\n",
    )
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

/// 仓库里出厂的模型资料读得出来，DeepSeek 官方的 `deepseek-flash` 在里面（施工 6-3 上；施工 8-6 随 `ModelTable` 挪进
/// `miyu-models` 以后，读文件的这一条留在核心：纯逻辑那一层的测试不读文件）。
#[test]
fn the_bundled_model_data_reads() {
    let table = ModelTable::parse(include_str!("../../../../resources/models/models-dev.json"))
        .expect("读得出来");
    assert_eq!(
        table.find("deepseek", "deepseek-flash"),
        ModelFacts {
            window: Some(1_000_000),
            max_output: Some(393_216)
        }
    );
}

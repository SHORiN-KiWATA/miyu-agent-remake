use super::*;

#[test]
fn deepseek_flash_views_images_but_not_pdf() {
    // DeepSeek 从 2026-08-21 起收图（施工 4-13）；PDF 不收。
    let call = call(model(MODEL));
    assert_eq!(call.model.as_str(), "deepseek-flash");
    assert!(call.inputs.images);
    assert!(!call.inputs.pdf);
    assert_eq!(call.max_output, None);
}

fn env(key: Option<&str>, base_url: Option<&str>, model: Option<&str>) -> ModelEnv {
    ModelEnv {
        key: key.map(str::to_string),
        base_url: base_url.map(str::to_string),
        model: model.map(str::to_string),
    }
}

/// 开发用的地址和模型（施工 3-9 再补）：没设的照旧接 DeepSeek 官方；设了的各替各的，换了地址的端点编号是 `dev`；
/// 空白当没设；没有 key 的不看它们；模型名不合写法的起不来。
#[test]
fn the_dev_variables_replace_the_address_and_the_model() {
    let chosen = |e: ModelEnv| choose(&e).unwrap().unwrap();
    let official = chosen(env(Some(" k "), None, None));
    assert_eq!(
        (
            official.base_url.as_str(),
            official.provider,
            official.model.as_str(),
            official.key.as_str()
        ),
        (
            "https://api.deepseek.com",
            "deepseek",
            "deepseek-flash",
            "k"
        )
    );
    let address = chosen(env(Some("k"), Some(" http://proxy.invalid/v1 "), None));
    assert_eq!(
        (
            address.base_url.as_str(),
            address.provider,
            address.model.as_str()
        ),
        ("http://proxy.invalid/v1", "dev", "deepseek-flash")
    );
    let model_only = chosen(env(Some("k"), None, Some("some-model-1.0")));
    assert_eq!(
        (
            model_only.base_url.as_str(),
            model_only.provider,
            model_only.model.as_str()
        ),
        ("https://api.deepseek.com", "deepseek", "some-model-1.0")
    );
    let both = chosen(env(
        Some("k"),
        Some("http://proxy.invalid/v1"),
        Some("some-model-1.0"),
    ));
    assert_eq!(
        (both.provider, both.model.as_str()),
        ("dev", "some-model-1.0")
    );
    assert_eq!(
        chosen(env(Some("k"), Some("  "), Some(" "))),
        official_with_key("k")
    );
    assert_eq!(choose(&env(None, Some("http://x/v1"), Some("m"))), Ok(None));
    assert_eq!(
        choose(&env(Some("  "), Some("http://x/v1"), Some("m"))),
        Ok(None)
    );
    let error = choose(&env(Some("k"), None, Some("bad\nname"))).unwrap_err();
    assert!(
        error.starts_with("MIYU_DEV_MODEL \"bad\\nname\" is not a model name"),
        "{error}"
    );
}

fn official_with_key(key: &str) -> Chosen {
    choose(&env(Some(key), None, None)).unwrap().unwrap()
}

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
        window: None,
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

/// `MIYU_DEV_WINDOW`（施工 6-3 上）：正整数压过模型资料里的窗口；不是正整数的起不来；空白当没设；没有 key 不看。开发端点
/// 照 `deepseek` 那一家查。
#[test]
fn the_dev_window_replaces_the_one_from_the_model_data() {
    let table = ModelTable::parse(
        r#"{"source":"x","fetched":"y","providers":{"deepseek":{"models":{"deepseek-flash":{"limit":{"context":1000000,"output":393216}},"some-model-1.0":{"limit":{"context":500000}}}}}}"#,
    )
    .unwrap();
    let with_window =
        |window: Option<&str>, base_url: Option<&str>, model: Option<&str>| ModelEnv {
            window: window.map(str::to_string),
            ..env(Some("k"), base_url, model)
        };
    let facts_of = |env: ModelEnv| facts(&choose(&env).unwrap().unwrap(), &table);
    assert_eq!(
        facts_of(with_window(None, None, None)),
        ModelFacts {
            window: Some(1_000_000),
            max_output: Some(393_216)
        }
    );
    assert_eq!(
        facts_of(with_window(Some(" 60000 "), None, None)),
        ModelFacts {
            window: Some(60_000),
            max_output: Some(393_216)
        }
    );
    assert_eq!(
        facts_of(with_window(Some("  "), None, None)).window,
        Some(1_000_000)
    );
    // 开发端点也照 deepseek 那一家查；查不到的，没有窗口，除非设了变量。
    assert_eq!(
        facts_of(with_window(
            None,
            Some("http://proxy.invalid/v1"),
            Some("some-model-1.0")
        )),
        ModelFacts {
            window: Some(500_000),
            max_output: None
        }
    );
    assert_eq!(
        facts_of(with_window(
            None,
            Some("http://proxy.invalid/v1"),
            Some("unknown-9")
        )),
        ModelFacts::default()
    );
    assert_eq!(
        facts_of(with_window(
            Some("60000"),
            Some("http://proxy.invalid/v1"),
            Some("unknown-9")
        ))
        .window,
        Some(60_000)
    );
    for bad in ["0", "-5", "6e4", "sixty"] {
        let error = choose(&with_window(Some(bad), None, None)).unwrap_err();
        assert!(error.starts_with("MIYU_DEV_WINDOW"), "{bad}: {error}");
    }
    let no_key = ModelEnv {
        window: Some("bad".to_string()),
        ..env(None, None, None)
    };
    assert_eq!(choose(&no_key), Ok(None));
}

/// 核心造的端口带着查到的窗口、最大输出，和 DeepSeek 的图片算法（施工 6-3 上）。
#[test]
fn the_port_carries_the_limits_and_the_deepseek_image_price() {
    use miyu_drivers::{DriverTextSources, DriverTexts, TextFileSources};
    use miyu_session::ForSession;
    use miyu_store::blob::Blobs;
    let table = ModelTable::parse(
        r#"{"source":"x","fetched":"y","providers":{"deepseek":{"models":{"deepseek-flash":{"limit":{"context":1000000,"output":393216}}}}}}"#,
    )
    .unwrap();
    let texts = DriverTexts::new(DriverTextSources {
        image_omitted: include_str!("../../../../resources/core/drivers/image-omitted.txt"),
        file_omitted: include_str!("../../../../resources/core/drivers/file-omitted.txt"),
        no_output: include_str!("../../../../resources/core/drivers/no-output.txt"),
        tool_attachments: include_str!("../../../../resources/core/drivers/tool-attachments.txt"),
        tool_attachments_only: include_str!(
            "../../../../resources/core/drivers/tool-attachments-only.txt"
        ),
        text_file: Some(TextFileSources {
            file_open: include_str!("../../../../resources/core/drivers/file-open.txt"),
            file_cut: include_str!("../../../../resources/core/drivers/file-cut.txt"),
            file_close: include_str!("../../../../resources/core/drivers/file-close.txt"),
        }),
    })
    .unwrap();
    let port = |env: &ModelEnv| {
        from_env(env, &table).unwrap().port(ForSession {
            texts: texts.clone(),
            blobs: Blobs::new(std::path::PathBuf::from("unused")),
        })
    };
    let limits = port(&env(Some("k"), None, None)).limits();
    assert_eq!(
        (limits.window, limits.max_output),
        (Some(1_000_000), Some(393_216))
    );
    let images = limits.images.expect("DeepSeek 的写法交图片算法");
    assert_eq!(images.tokens(1920, 1080), 968);
    // 没有 key 的，什么限额都没有：不主动压。
    let none = port(&env(None, None, None)).limits();
    assert_eq!((none.window, none.images.is_none()), (None, true));
}

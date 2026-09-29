//! 模型资料的读法和查法（施工 6-3 上）。

use super::*;

const SAMPLE: &str = r#"{"source":"https://models.dev/api.json","fetched":"2026-09-29","providers":{"deepseek":{"models":{"deepseek-flash":{"limit":{"context":1000000,"output":393216}},"half":{"limit":{"context":64000}}}}}}"#;

#[test]
fn a_model_is_found_by_provider_and_name() {
    let table = ModelTable::parse(SAMPLE).unwrap();
    assert_eq!(
        table.find("deepseek", "deepseek-flash"),
        ModelFacts {
            window: Some(1_000_000),
            max_output: Some(393_216)
        }
    );
    // 只写了窗口的，最大输出没有。
    assert_eq!(
        table.find("deepseek", "half"),
        ModelFacts {
            window: Some(64_000),
            max_output: None
        }
    );
    // 查不到的都是没有：别的供应商、别的模型。
    assert_eq!(table.find("dev", "deepseek-flash"), ModelFacts::default());
    assert_eq!(table.find("deepseek", "nope"), ModelFacts::default());
}

#[test]
fn a_broken_file_says_it_is_the_model_data() {
    for text in [
        "",
        "{}",
        r#"{"source":"x","fetched":"y","providers":{"deepseek":{"models":{"m":{"limit":{"context":-1}}}}}}"#,
    ] {
        let error = ModelTable::parse(text).unwrap_err();
        assert!(
            error.starts_with("models/models-dev.json not readable: "),
            "{error}"
        );
    }
}

/// 仓库里出厂的那一份读得出来，DeepSeek 官方的 `deepseek-flash` 在里面。
#[test]
fn the_bundled_file_reads() {
    let path = std::path::Path::new(miyu_base_root()).join("resources/models/models-dev.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let table = ModelTable::parse(&text).unwrap();
    assert_eq!(
        table.find("deepseek", "deepseek-flash"),
        ModelFacts {
            window: Some(1_000_000),
            max_output: Some(393_216)
        }
    );
}

/// 仓库根：这个 crate 往上两级。
fn miyu_base_root() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../..")
}

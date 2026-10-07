//! 键的样子和真的键（施工 8-6）：占位的写法、拆开接上互为来回、对样子、列名字。

use super::*;

fn segments(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

#[test]
fn an_id_is_a_path_name_and_a_model_is_short_text() {
    for good in ["deepseek", "dev", "a", "open-router_2", &"a".repeat(32)] {
        assert!(valid(ID, good), "{good}");
    }
    for bad in [
        "",
        "DeepSeek",
        "1st",
        "-a",
        "a.b",
        "a b",
        "con",
        &"a".repeat(33),
    ] {
        assert!(!valid(ID, bad), "{bad}");
    }
    for good in [
        "deepseek-v4.1-flash",
        "deepseek/deepseek-v4",
        "DeepSeek V4",
        "x",
    ] {
        assert!(valid(MODEL, good), "{good}");
    }
    for bad in ["", "a\nb", &"m".repeat(129)] {
        assert!(!valid(MODEL, bad), "{bad:?}");
    }
    assert!(!valid("<other>", "a"), "不是占位的不合");
}

#[test]
fn keys_split_and_join_back_with_quotes_only_where_needed() {
    let key = r#"providers.dev.models."deepseek-v4.1-flash".window"#;
    let parts = split(key).expect("读得成");
    assert_eq!(
        parts,
        segments(&[
            "providers",
            "dev",
            "models",
            "deepseek-v4.1-flash",
            "window"
        ])
    );
    assert_eq!(join(&parts), key);
    assert_eq!(join(&["ui", "language"]), "ui.language");
    assert_eq!(
        join(&["m", "deepseek/x y", "中"]),
        r#"m."deepseek/x y"."中""#
    );
    assert_eq!(split("a . 'b.c' . d"), Some(segments(&["a", "b.c", "d"])));
    assert_eq!(split("a..b"), None);
    assert_eq!(split(""), None);
    assert_eq!(split("a = 1"), None);
}

#[test]
fn a_key_fits_a_pattern_segment_by_segment() {
    let pattern = "providers.<id>.models.<model>.window";
    assert_eq!(
        fit(
            pattern,
            &segments(&["providers", "dev", "models", "a.b", "window"])
        ),
        Fit::Yes(segments(&["dev", "a.b"]))
    );
    assert_eq!(
        fit(
            pattern,
            &segments(&["providers", "Dev", "models", "m", "window"])
        ),
        Fit::BadName {
            at: 1,
            placeholder: ID
        }
    );
    assert_eq!(
        fit(
            pattern,
            &segments(&["providers", "dev", "models", "m", "size"])
        ),
        Fit::No
    );
    assert_eq!(fit(pattern, &segments(&["providers", "dev"])), Fit::No);
    assert_eq!(
        fit("ui.language", &segments(&["ui", "language"])),
        Fit::Yes(Vec::new())
    );
    assert_eq!(
        fit_start(pattern, &segments(&["providers", "dev"])),
        Fit::Yes(segments(&["dev"]))
    );
    assert_eq!(
        fit_start(pattern, &segments(&["providers", "-x"])),
        Fit::BadName {
            at: 1,
            placeholder: ID
        }
    );
    assert_eq!(
        fit_start("ui.language", &segments(&["ui", "language"])),
        Fit::No
    );
}

#[test]
fn fill_puts_names_into_the_placeholders() {
    assert_eq!(
        fill("providers.<id>.driver", &["dev"]),
        "providers.dev.driver"
    );
    assert_eq!(
        fill("providers.<id>.models.<model>.window", &["dev", "v4.1"]),
        r#"providers.dev.models."v4.1".window"#
    );
    assert_eq!(fill("ui.language", &[]), "ui.language");
    assert!(is_pattern("providers.<id>.driver"));
    assert!(!is_pattern("ui.language"));
}

#[test]
fn names_lists_what_fills_the_next_placeholder() {
    let keys = [
        "providers.dev.base_url",
        "providers.dev.keys",
        "providers.deepseek.keys",
        r#"providers.dev.models."v4.1".window"#,
        "models.chat",
    ];
    let keys = keys.iter().copied();
    assert_eq!(
        names(keys.clone(), "providers.<id>", &[]),
        segments(&["deepseek", "dev"])
    );
    assert_eq!(
        names(keys.clone(), "providers.<id>.models.<model>", &["dev"]),
        segments(&["v4.1"])
    );
    assert!(names(keys, "providers.<id>.models.<model>", &["deepseek"]).is_empty());
}

/// 平台身份的占位（施工 O-3）：照短名字的写法，冒号也行；空的、太长的、有控制字符的不行。
#[test]
fn an_external_identity_is_short_text() {
    for good in ["qq:10001", "a", &"q".repeat(128)] {
        assert!(valid(EXTERNAL, good), "{good}");
    }
    for bad in ["", "qq:\u{7}1", &"q".repeat(129)] {
        assert!(!valid(EXTERNAL, bad), "{bad}");
    }
    assert!(is_placeholder(EXTERNAL));
    assert_eq!(
        fit(
            "external.bindings.<external>",
            &segments(&["external", "bindings", "qq:10001"])
        ),
        Fit::Yes(vec!["qq:10001".to_string()])
    );
}

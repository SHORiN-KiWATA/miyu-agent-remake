//! 四层对目录（施工 8-7）：「怎么走」第二条第 7 条那张表的每一行，规整，单段通用名，几家同名先取第 2 层认出的那家、再
//! 原厂、再字节序且不借价格。目录是真目录裁出来的一份（`test_support`）。

use serde_json::json;

use super::*;
use crate::test_support::{trimmed, vendors};

fn matched(provider: &str, model: &str, layer: u8, price: bool) -> Found {
    Found::Matched(Matched {
        provider: provider.to_string(),
        model: model.to_string(),
        layer,
        price,
    })
}

/// 配好的一家 `id`（地址 `base_url`）的模型 `model`，模型手写指定 `written`。
fn found(id: &str, base_url: Option<&str>, model: &str, written: Option<&str>) -> Found {
    let catalog = trimmed();
    let recognized = recognize(&catalog, id, base_url, None);
    find(
        &catalog,
        &vendors(),
        recognized.as_ref().map(|r| r.provider.as_str()),
        model,
        written,
    )
}

/// 图纸那张表，一行一个。`newapi` 是自己配的中转，目录里认不出。
#[test]
fn each_row_of_the_example_table() {
    let relay = Some("https://relay.example.invalid/v1");
    assert_eq!(
        found("opencodego", None, "deepseek-v4.1-flash", None),
        matched("opencode-go", "deepseek-v4.1-flash", 2, true),
        "编号去掉 - 一样"
    );
    assert_eq!(
        found(
            "deepseek",
            Some("https://api.deepseek.com/v1"),
            "deepseek-flash",
            None
        ),
        matched("deepseek", "deepseek-flash", 2, true),
        "编号一样"
    );
    assert_eq!(
        found("newapi", relay, "deepseek-v4.1-flash", None),
        matched("above", "deepseek-v4.1-flash", 3, false),
        "十几家都不是原厂：取编号排第一的，价格不借"
    );
    assert_eq!(
        found("newapi", relay, "claude-sonnet-4-5", None),
        matched("anthropic", "claude-sonnet-4-5", 3, true),
        "原厂列了：不取字节序第一的 aihubmix"
    );
    assert_eq!(
        found("newapi", relay, "deepseek-flash-expire-xxxx", None),
        matched("deepseek", "deepseek-flash", 4, true),
        "前缀，原厂列了"
    );
    assert_eq!(
        found("newapi", relay, "DeepSeek V4 Flash", None),
        matched("deepseek", "deepseek-v4-flash", 4, true),
        "大小写、空格不分"
    );
    assert_eq!(
        found("newapi", relay, "gpt-5-minimal", None),
        matched("openai", "gpt-5", 4, true),
        "gpt-5-mini 不在分隔处断开"
    );
    assert_eq!(
        found("newapi", relay, "custom-7b", None),
        Found::Nothing,
        "custom 是单段通用名"
    );
    assert_eq!(
        found("newapi", relay, "x", Some("deepseek/nope")),
        Found::Missing("deepseek/nope".to_string()),
        "手写指定的不存在，不往下猜"
    );
}

#[test]
fn a_hand_picked_entry_wins_and_may_hold_a_slash() {
    assert_eq!(
        found(
            "newapi",
            None,
            "anything",
            Some("opencode-go/deepseek-v4.1-flash")
        ),
        matched("opencode-go", "deepseek-v4.1-flash", 1, true)
    );
    assert_eq!(
        found("x", None, "y", Some("deepinfra/tencent/Hy3")),
        matched("deepinfra", "tencent/Hy3", 1, true),
        "在第一个 / 处切开"
    );
    assert_eq!(
        found("x", None, "y", Some("no-slash")),
        Found::Missing("no-slash".to_string())
    );
}

#[test]
fn normalizing_keeps_the_last_part_and_folds_separators() {
    for (name, normal) in [
        ("DeepSeek V4 Flash", "deepseek-v4-flash"),
        ("deepseek-v4.1-flash", "deepseek-v4-1-flash"),
        ("cline-pass/deepseek-v4.1-flash", "deepseek-v4-1-flash"),
        ("  --Qwen__3.5 . Plus--  ", "qwen-3-5-plus"),
        ("a/b/", ""),
        ("", ""),
    ] {
        assert_eq!(normalize(name), normal, "{name:?}");
    }
}

/// 一家在目录里是哪一家：手写的、编号、去掉分隔的编号、地址，先对上的算。
#[test]
fn a_provider_is_recognized_in_order() {
    let catalog = trimmed();
    let how = |id: &str, url: Option<&str>, written: Option<&str>| {
        recognize(&catalog, id, url, written).map(|r| (r.provider, r.how))
    };
    let at = |provider: &str, how: How| Some((provider.to_string(), how));
    assert_eq!(
        how("dev", None, Some("deepseek")),
        at("deepseek", How::Config)
    );
    assert_eq!(
        how("deepseek", None, Some("nope")),
        None,
        "手写的目录里没有：当认不出"
    );
    assert_eq!(how("DeepSeek", None, None), at("deepseek", How::Id));
    assert_eq!(
        how("open.code_go", None, None),
        at("opencode-go", How::SimilarId)
    );
    assert_eq!(
        how("relay", Some("HTTPS://API.DEEPSEEK.COM/v1/"), None),
        at("deepseek", How::Url),
        "去掉 /v1 和末尾的 /，协议和主机名不分大小写"
    );
    assert_eq!(
        how("relay", Some("https://api.deepseek.com/beta"), None),
        None
    );
    assert_eq!(how("relay", None, None), None);
    assert_eq!(How::SimilarId.as_str(), "similar_id");
}

/// 几家同名：第 2 层认出的那一家先于原厂，原厂先于字节序。
#[test]
fn the_recognized_provider_comes_before_the_vendor() {
    let catalog = trimmed();
    let pick =
        |recognized: Option<&str>, model: &str| find(&catalog, &vendors(), recognized, model, None);
    // opencode 认出来了，模型名没列一模一样的：前缀那一层先取它，价格照它的（它是你用的那一家）。
    assert_eq!(
        pick(Some("opencode"), "deepseek-v4.1-flash-preview"),
        matched("opencode", "deepseek-v4.1-flash", 4, true)
    );
    // 认出的那一家没列它：原厂。
    assert_eq!(
        pick(Some("302ai"), "claude-sonnet-4-5"),
        matched("anthropic", "claude-sonnet-4-5", 3, true)
    );
    // 第 2 层也认规整以后一样的名字。
    assert_eq!(
        pick(Some("deepseek"), "DeepSeek-Flash"),
        matched("deepseek", "deepseek-flash", 2, true)
    );
}

#[test]
fn a_one_word_name_without_digits_only_matches_exactly() {
    let catalog = Catalog::parse(
        &json!({"a": {"models": {"fast": {}, "fast-2": {}}}, "b": {"models": {"custom": {}}}})
            .to_string(),
    )
    .expect("读得进")
    .catalog;
    let find = |model: &str| find(&catalog, &vendors(), None, model, None);
    assert_eq!(
        find("Custom"),
        matched("b", "custom", 4, false),
        "正好相等的算"
    );
    assert_eq!(find("fast-coder"), Found::Nothing);
    assert_eq!(
        find("fast-2-turbo"),
        matched("a", "fast-2", 4, false),
        "有数字的不算通用名"
    );
}

#[test]
fn the_vendor_table_reads_and_strips_trailing_digits() {
    let table = vendors();
    assert_eq!(table.of(Some("qwen3"), "x"), ["alibaba", "alibaba-cn"]);
    assert_eq!(table.of(None, "o3-mini"), ["openai"]);
    assert_eq!(table.of(Some("claude-sonnet"), "x"), ["anthropic"]);
    assert!(table.of(Some("hy"), "x").is_empty());
    let error = Vendors::parse(&json!({"gpt": "openai"})).expect_err("读不进");
    assert!(
        error.starts_with("models/vendors.toml not readable: "),
        "{error}"
    );
}

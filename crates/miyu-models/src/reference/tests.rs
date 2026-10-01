//! 三种写法（`models.md`「守着它的」第一行，施工 8-6）：先后、切在第一个 `/`、哪里能写哪几种。挡位退回 `chat`、池的认不出
//! 的成员跳过随 8-8。

use super::*;

fn model(provider: &str, model: &str) -> Reference {
    Reference::Model {
        provider: provider.to_string(),
        model: model.to_string(),
    }
}

#[test]
fn the_three_ways_are_read_in_order() {
    assert_eq!(
        Reference::parse("@free"),
        Ok(Reference::Pool("free".to_string()))
    );
    for tier in TIERS {
        assert_eq!(Reference::parse(tier), Ok(Reference::Tier(tier)));
    }
    assert_eq!(
        Reference::parse("deepseek/deepseek-flash"),
        Ok(model("deepseek", "deepseek-flash"))
    );
    // `@` 在前：`@lite` 是池，不是挡位；挡位只认正好是那几个词的。
    assert_eq!(
        Reference::parse("@lite"),
        Ok(Reference::Pool("lite".to_string()))
    );
    assert_eq!(
        Reference::parse("Lite"),
        Err(Bad::NotAReference("Lite".to_string()))
    );
}

#[test]
fn a_model_is_cut_at_the_first_slash() {
    assert_eq!(
        Reference::parse("openrouter/deepseek/deepseek-v4"),
        Ok(model("openrouter", "deepseek/deepseek-v4"))
    );
    assert_eq!(
        Reference::parse("newapi/DeepSeek V4 Flash"),
        Ok(model("newapi", "DeepSeek V4 Flash"))
    );
    assert_eq!(Reference::parse("dev/v4.1"), Ok(model("dev", "v4.1")));
    for bad in [
        "", "deepseek", "/m", "dev/", "Dev/m", "dev m/x", "@", "@Free", "dev/a\nb",
    ] {
        assert_eq!(
            Reference::parse(bad),
            Err(Bad::NotAReference(bad.to_string())),
            "{bad:?}"
        );
    }
    assert_eq!(model("openrouter", "a/b").to_string(), "openrouter/a/b");
    assert_eq!(Reference::Pool("free".to_string()).to_string(), "@free");
    assert_eq!(Reference::Tier("lite").to_string(), "lite");
}

#[test]
fn each_place_takes_only_its_kinds() {
    assert_eq!(
        Reference::parse_at("cheap", Place::Use),
        Err(Bad::TierHere("cheap".to_string()))
    );
    assert!(Reference::parse_at("@free", Place::Use).is_ok());
    assert!(Reference::parse_at("dev/m", Place::Use).is_ok());
    assert_eq!(
        Reference::parse_at("@free", Place::PoolMember),
        Err(Bad::NotAModel("@free".to_string()))
    );
    assert_eq!(
        Reference::parse_at("lite", Place::PoolMember),
        Err(Bad::NotAModel("lite".to_string()))
    );
    assert!(Reference::parse_at("dev/m", Place::PoolMember).is_ok());
    for text in ["lite", "@free", "dev/m"] {
        assert!(Reference::parse_at(text, Place::Session).is_ok(), "{text}");
    }
}

#[test]
fn the_errors_say_what_the_blueprint_says() {
    assert_eq!(
        Bad::NotAReference("x".to_string()).to_string(),
        r#""x" is not a model, a pool or a tier"#
    );
    assert_eq!(
        Bad::TierHere("lite".to_string()).to_string(),
        r#"a tier cannot be used here: "lite""#
    );
    assert_eq!(
        Bad::NotAModel("@p".to_string()).to_string(),
        r#"pool members must be models: "@p""#
    );
}

/// 配置里引用的写法（`miyu_config::Kind::Reference`）和这里认的一样：配置收下的，这里在 `Use` 那一处也认得。
#[test]
fn the_config_accepts_what_a_use_place_accepts() {
    let kind = miyu_config::Kind::Reference;
    for text in [
        "dev/m",
        "a/b/c",
        "dev/DeepSeek V4",
        "@free",
        "lite",
        "m",
        "",
        "Dev/m",
        "@",
        "dev/",
    ] {
        let value = miyu_config::Value::Text(text.to_string().into());
        assert_eq!(
            kind.accepts(&value),
            Reference::parse_at(text, Place::Use).is_ok(),
            "{text:?}"
        );
    }
}

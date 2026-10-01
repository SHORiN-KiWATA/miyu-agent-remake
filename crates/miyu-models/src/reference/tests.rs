//! 两种写法（`models.md`「守着它的」第一行，施工 8-6）：先后、切在第一个 `/`、哪里能写哪几种。施工 8-8：池的认不出的成员
//! 跳过；造会话记下的是查过的模型或池；一个引用这一轮指到哪。施工 8-8 补：以前的挡位名哪里都不认。

use serde_json::json;

use super::*;
use crate::pools::{Member, Strategy};
use crate::provider::Driver;
use crate::test_support::{Held, resolved};

fn model(provider: &str, model: &str) -> Reference {
    Reference::Model {
        provider: provider.to_string(),
        model: model.to_string(),
    }
}

#[test]
fn the_two_ways_are_read_in_order() {
    assert_eq!(
        Reference::parse("@free"),
        Ok(Reference::Pool("free".to_string()))
    );
    assert_eq!(
        Reference::parse("deepseek/deepseek-flash"),
        Ok(model("deepseek", "deepseek-flash"))
    );
    assert_eq!(
        Reference::parse("@lite"),
        Ok(Reference::Pool("lite".to_string()))
    );
    // 以前的挡位名（施工 8-8 补去掉了）：照写法不对。
    for tier in ["lite", "cheap", "standard", "flagship", "Lite"] {
        assert_eq!(
            Reference::parse(tier),
            Err(Bad::NotAReference(tier.to_string())),
            "{tier}"
        );
    }
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
}

#[test]
fn each_place_takes_only_its_kinds() {
    assert_eq!(
        Reference::parse_at("cheap", Place::Use),
        Err(Bad::NotAReference("cheap".to_string()))
    );
    assert!(Reference::parse_at("@free", Place::Use).is_ok());
    assert!(Reference::parse_at("dev/m", Place::Use).is_ok());
    assert_eq!(
        Reference::parse_at("@free", Place::PoolMember),
        Err(Bad::NotAModel("@free".to_string()))
    );
    assert_eq!(
        Reference::parse_at("lite", Place::PoolMember),
        Err(Bad::NotAReference("lite".to_string()))
    );
    assert!(Reference::parse_at("dev/m", Place::PoolMember).is_ok());
}

#[test]
fn the_errors_say_what_the_blueprint_says() {
    assert_eq!(
        Bad::NotAReference("x".to_string()).to_string(),
        r#""x" is not a model or a pool"#
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

/// 池的成员只收模型（`Kind::Model`），配置收的和 `PoolMember` 那一处认的一样（施工 8-8）。
#[test]
fn the_config_accepts_as_a_pool_member_what_a_pool_member_place_accepts() {
    let kind = miyu_config::Kind::Model;
    for text in ["dev/m", "a/b/c", "@free", "lite", "m", "", "Dev/m", "dev/"] {
        let value = miyu_config::Value::Text(text.to_string().into());
        assert_eq!(
            kind.accepts(&value),
            Reference::parse_at(text, Place::PoolMember).is_ok(),
            "{text:?}"
        );
    }
}

/// 两家、一个池，`models.chat` 是 `a/main`、`models.vision` 是 `b/big`。
const CONFIG: &str = "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"https://a.invalid\"\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"https://b.invalid\"\n\n[models]\nchat = \"a/main\"\nvision = \"b/big\"\n\n[pools.free]\nmodels = [\"a/x\", \"gone/y\", \"b/z\"]\n";

fn values(source: &str) -> Values {
    resolved(source).values()
}

/// 用途、池里点名的模型（`model.list` 照它列）：照 `chat`、`vision`、池的成员的先后，池、写法不对的不算（施工 8-8 补：
/// 没有挡位了）。
#[test]
fn named_models_come_from_the_uses_and_the_pools() {
    let named = named(&values(CONFIG));
    let named: Vec<String> = named
        .into_iter()
        .map(|(provider, model)| format!("{provider}/{model}"))
        .collect();
    assert_eq!(named, ["a/main", "b/big", "a/x", "gone/y", "b/z"]);
}

#[test]
fn a_session_records_the_model_or_pool_a_reference_resolves_to_now() {
    let values = values(CONFIG);
    let record = |text: &str| record(&values, text);
    assert_eq!(
        record("b/anything").as_deref(),
        Ok("b/anything"),
        "模型名不查"
    );
    assert_eq!(record("@free").as_deref(), Ok("@free"));
    for (text, why) in [
        ("c/m", r#"no provider "c""#),
        ("@paid", r#"no pool "paid""#),
        ("nope", r#""nope" is not a model or a pool"#),
        ("lite", r#""lite" is not a model or a pool"#),
        ("flagship", r#""flagship" is not a model or a pool"#),
    ] {
        assert_eq!(record(text), Err(NoModel(why.to_string())), "{text}");
    }
    let bare =
        resolved("[providers.a]\nkeys = []\n\n[pools.empty]\nmodels = [\"gone/y\"]\n").values();
    assert_eq!(
        crate::reference::record(&bare, "@empty"),
        Err(NoModel(r#"pool "empty" has no models"#.to_string()))
    );
}

#[test]
fn a_reference_resolves_to_a_model_or_a_pool_this_turn() {
    let values = values(CONFIG);
    let held = Held::new(json!({}), false);
    let knowledge = held.knowledge();
    match resolve(&values, &knowledge, "b/big") {
        Ok(Resolved::Model(target)) => {
            assert_eq!(
                (target.provider.id.as_str(), target.model.as_str()),
                ("b", "big")
            );
            assert_eq!(target.provider.driver, Driver::OpenAiChat);
        }
        other => panic!("{other:?}"),
    }
    match resolve(&values, &knowledge, "@free") {
        Ok(Resolved::Pool(pool)) => {
            assert_eq!(pool.name, "free");
            assert_eq!(pool.strategy, Strategy::Pin);
            assert_eq!(
                pool.members,
                [
                    Member {
                        provider: "a".to_string(),
                        model: "x".to_string()
                    },
                    Member {
                        provider: "b".to_string(),
                        model: "z".to_string()
                    },
                ],
                "认不出的成员跳过，别的照写的先后"
            );
            assert_eq!(pool.skipped, ["gone/y"]);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(
        resolve(&values, &knowledge, "cheap"),
        Err(NoModel(r#""cheap" is not a model or a pool"#.to_string())),
        "以前的挡位名不认"
    );
    assert_eq!(
        resolve(&values, &knowledge, "c/m"),
        Err(NoModel(r#"no provider "c""#.to_string()))
    );
    assert_eq!(
        resolve(&Values::default(), &knowledge, "@free"),
        Err(NoModel(r#"no pool "free""#.to_string()))
    );
    assert_eq!(
        crate::provider::NOT_CONFIGURED,
        "no model configured: set models.chat"
    );
}

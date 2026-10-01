//! 三种写法（`models.md`「守着它的」第一行，施工 8-6）：先后、切在第一个 `/`、哪里能写哪几种。施工 8-8：挡位退回 `chat`、
//! 不借相邻的；池的认不出的成员跳过；造会话记下的是解析出的模型或池；一个引用这一轮指到哪。

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

/// 两家、一个池、两个挡位配了：`lite` 是池，`flagship` 是模型，另两挡没配。
const CONFIG: &str = "[providers.a]\ndriver = \"openai-chat\"\nbase_url = \"https://a.invalid\"\n\n[providers.b]\ndriver = \"openai-chat\"\nbase_url = \"https://b.invalid\"\n\n[models]\nchat = \"a/main\"\n\n[models.tiers]\nlite = \"@free\"\nflagship = \"b/big\"\n\n[pools.free]\nmodels = [\"a/x\", \"gone/y\", \"b/z\"]\n";

fn values(source: &str) -> Values {
    resolved(source).values()
}

#[test]
fn a_tier_is_its_value_or_the_chat_model_never_a_neighbour() {
    let values = values(CONFIG);
    assert_eq!(tier(&values, "lite").as_deref(), Some("@free"));
    assert_eq!(tier(&values, "flagship").as_deref(), Some("b/big"));
    for unset in ["cheap", "standard"] {
        assert_eq!(
            tier(&values, unset).as_deref(),
            Some("a/main"),
            "{unset} 没配：用 chat，不借相邻的"
        );
    }
    assert_eq!(tier(&values, "huge"), None, "不是挡位");
    assert_eq!(tier(&Values::default(), "lite"), None, "chat 也没配");
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
    assert_eq!(
        record("lite").as_deref(),
        Ok("@free"),
        "挡位记下的是它这时的值"
    );
    assert_eq!(record("flagship").as_deref(), Ok("b/big"));
    assert_eq!(record("cheap").as_deref(), Ok("a/main"), "没配的用 chat");
    for (text, why) in [
        ("c/m", r#"no provider "c""#),
        ("@paid", r#"no pool "paid""#),
        ("nope", r#""nope" is not a model, a pool or a tier"#),
    ] {
        assert_eq!(record(text), Err(NoModel(why.to_string())), "{text}");
    }
    let bare =
        resolved("[providers.a]\nkeys = []\n\n[pools.empty]\nmodels = [\"gone/y\"]\n").values();
    assert_eq!(
        crate::reference::record(&bare, "@empty"),
        Err(NoModel(r#"pool "empty" has no models"#.to_string()))
    );
    assert_eq!(
        crate::reference::record(&bare, "lite"),
        Err(NoModel(NOT_CONFIGURED.to_string())),
        "挡位没配又没有 chat"
    );
}

#[test]
fn a_reference_resolves_to_a_model_or_a_pool_this_turn() {
    let values = values(CONFIG);
    let held = Held::new(json!({}), false);
    let knowledge = held.knowledge();
    match resolve(&values, &knowledge, "flagship") {
        Ok(Resolved::Model(target)) => {
            assert_eq!(
                (target.provider.id.as_str(), target.model.as_str()),
                ("b", "big")
            );
            assert_eq!(target.provider.driver, Driver::OpenAiChat);
        }
        other => panic!("{other:?}"),
    }
    match resolve(&values, &knowledge, "lite") {
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
        resolve(&values, &knowledge, "a/main"),
        "没配的挡位用 chat"
    );
    assert_eq!(
        resolve(&values, &knowledge, "c/m"),
        Err(NoModel(r#"no provider "c""#.to_string()))
    );
    assert_eq!(
        resolve(&Values::default(), &knowledge, "lite"),
        Err(NoModel(NOT_CONFIGURED.to_string()))
    );
    assert_eq!(NOT_CONFIGURED, "no model configured: set models.chat");
}

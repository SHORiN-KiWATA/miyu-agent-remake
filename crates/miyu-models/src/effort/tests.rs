//! 思考强度（施工 8-18；8-18（补）去掉会话那一层）：档位名规整、开关照档案、给头看的那一档从配置的哪一层来、空闲超时
//! 放大几倍、配置里写的不在档位里的。

use miyu_config::parse::parse;
use serde_json::json;

use super::*;
use crate::facts::{Fact, Source, facts};
use crate::provider::provider;
use crate::settings::ModelSettings;
use crate::test_support::{Held, items, resolved};

fn names(levels: &[&str]) -> Vec<String> {
    levels.iter().map(|level| (*level).to_string()).collect()
}

fn reasoning(levels: &[&str], toggle: bool) -> Reasoning {
    Reasoning {
        levels: names(levels),
        toggle,
    }
}

#[test]
fn none_and_disabled_read_as_off_and_repeats_are_dropped() {
    assert_eq!(normalize("none"), "off");
    assert_eq!(normalize("disabled"), "off");
    assert_eq!(normalize("minimal"), "minimal");
    assert_eq!(
        levels(&names(&["none", "low", "disabled", "low", "high"])),
        names(&["off", "low", "high"])
    );
}

#[test]
fn the_toggle_counts_only_where_the_profile_can_say_it() {
    let both = reasoning(&["low", "high", "max"], true);
    assert_eq!(
        offered(&both, true),
        Some(names(&["off", "low", "high", "max"])),
        "能关的多一档 off，在最前面"
    );
    assert_eq!(
        offered(&both, false),
        Some(names(&["low", "high", "max"])),
        "档案没写开关的，目录的开关不算"
    );
    assert_eq!(
        offered(&reasoning(&[], true), true),
        Some(names(&["off", "on"])),
        "只有开关的是 off、on"
    );
    assert_eq!(offered(&reasoning(&[], true), false), None);
    assert_eq!(
        offered(&reasoning(&["off", "low"], true), true),
        Some(names(&["off", "low"])),
        "已经有 off 的不再加"
    );
    assert_eq!(
        offered(&reasoning(&["off", "low"], false), false),
        Some(names(&["off", "low"])),
        "目录写 none 的照样能关"
    );
    assert_eq!(
        offered(&reasoning(&["low"], false), true),
        Some(names(&["low"]))
    );
}

#[test]
fn in_use_reports_which_config_layer_set_it() {
    let fact = |value: Option<&str>, source: Source| Fact {
        value: value.map(str::to_string),
        source,
    };
    assert_eq!(
        in_use(&fact(
            Some("high"),
            Source::Config {
                layer: Layer::System,
                line: 3
            }
        )),
        Some(EffortInUse {
            level: "high".to_string(),
            from: EffortSource::System,
        }),
    );
    assert_eq!(
        in_use(&fact(
            Some("low"),
            Source::Config {
                layer: Layer::Personal,
                line: 1
            }
        )),
        Some(EffortInUse {
            level: "low".to_string(),
            from: EffortSource::Personal,
        }),
    );
    assert_eq!(
        in_use(&fact(None, Source::Default)),
        None,
        "没写就不带，不管来源"
    );
    assert_eq!(
        in_use(&fact(Some("high"), Source::Local)),
        None,
        "没写在配置文件里的不带（不会真出现：effort 不借目录、不是本机免费价）"
    );
}

#[test]
fn the_idle_timeout_grows_with_the_level() {
    assert_eq!(idle_factor(None), 1);
    for level in ["off", "on", "minimal", "low", "medium", "default"] {
        assert_eq!(idle_factor(Some(level)), 1, "{level}");
    }
    assert_eq!(idle_factor(Some("high")), 2);
    assert_eq!(idle_factor(Some("xhigh")), 3);
    assert_eq!(idle_factor(Some("max")), 4);
}

#[test]
fn the_item_is_the_one_the_settings_register() {
    assert!(ModelSettings::ITEMS.iter().any(|item| item.key == ITEM));
}

/// 档案里 DeepSeek 写了开关；裁出来的目录里 `deepseek-flash` 有开关和三档。
fn held() -> Held {
    Held::new(
        json!({
            "npm": {"@ai-sdk/openai-compatible": "openai-chat"},
            "providers": {"deepseek": {"driver": "openai-chat", "base_url": "https://api.deepseek.com",
                "compat": {"toggle": {"field": "thinking", "on": {"type": "enabled"}, "off": {"type": "disabled"}}}}}
        }),
        true,
    )
}

/// 照配置 `source` 查 `id` 这一家的 `model` 这时有哪几档；那一家用不了的说不出来。
fn levels_of(source: &str) -> impl Fn(&str, &str) -> Option<Vec<String>> {
    let resolved = resolved(source);
    let held = held();
    move |id: &str, model: &str| {
        let knowledge = held.knowledge();
        let provider = provider(&resolved.values(), &knowledge, id).ok()?;
        let (facts, _) = facts(&resolved, &knowledge, &provider, model);
        Some(facts.levels().to_vec())
    }
}

#[test]
fn a_written_level_the_model_does_not_have_is_reported_where_it_is_written() {
    let source = "[providers.deepseek]\nlocal = false\n\n[providers.deepseek.models.\"deepseek-flash\"]\neffort = \"medium\"\n\n[providers.deepseek.models.\"deepseek-v4-pro\"]\neffort = \"off\"\n\n[providers.deepseek.models.gone]\neffort = \"high\"\n\n[providers.broken.models.x]\neffort = \"high\"\n";
    let parsed = parse(&items(), Layer::System, source).expect("写法对");
    let found = unknown(&parsed, Layer::System, &levels_of(source));
    assert_eq!(found.len(), 2, "{found:?}");
    let first = &found[0];
    assert_eq!(first.code, Code::UnknownEffort);
    assert_eq!(first.code.as_str(), "unknown_effort");
    assert_eq!(
        first.key.as_deref(),
        Some("providers.deepseek.models.deepseek-flash.effort")
    );
    assert_eq!(first.name.as_deref(), Some("medium"));
    assert_eq!(first.at.map(|at| at.line), Some(5), "指到值那一行");
    assert_eq!(
        (found[1].key.as_deref(), found[1].name.as_deref()),
        (Some("providers.deepseek.models.gone.effort"), Some("high")),
        "目录里没有的模型一档都没有"
    );
    // 在档位里的（off 是开关加出来的一档）、那一家用不了的（broken 推不出驱动）不报。
    let none = "[providers.deepseek.models.\"deepseek-flash\"]\neffort = \"none\"\n";
    let parsed = parse(&items(), Layer::System, none).expect("写法对");
    let source = format!("[providers.deepseek]\nlocal = false\n\n{none}");
    assert!(
        unknown(&parsed, Layer::System, &levels_of(&source)).is_empty(),
        "none 读成 off，照样在档位里"
    );
}

#[test]
fn a_level_written_where_it_does_not_count_is_not_checked() {
    let source = "[providers.deepseek.models.\"deepseek-flash\"]\neffort = \"medium\"\n";
    let parsed = parse(&items(), Layer::Project, source).expect("写法对");
    let config = "[providers.deepseek]\nlocal = false\n";
    assert!(
        unknown(&parsed, Layer::Project, &levels_of(config)).is_empty(),
        "项目配置里写的本来就不算（wrong_layer 报过了）"
    );
}

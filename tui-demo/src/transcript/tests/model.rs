//! 换模型和冷却（蓝图 `tui.md`「配置与模型」第 2、7 条，核心 8-9）。

use jiff::Timestamp;

use super::super::{Kind, Transcript};
use super::apply;
use crate::config::Config;
use crate::core::{CallError, Current, Limits, Push, Update};

fn failed(class: &str) -> Push {
    Push::CallFailed(CallError {
        class: class.into(),
        message: String::new(),
        status: None,
    })
}

#[test]
fn a_failover_changes_the_model_and_says_why_in_a_dim_line() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::Model {
                endpoint: "deepseek".into(),
                model: "deepseek-v4".into(),
            },
            // 2026-10-01 实测：出错的是池里第一个，答话的是第二个；换端点的推送在答完以后才来，那时底栏已经照答话换成
            // 第二个了。「原来的」照出错那次请求的端点、模型写。
            Push::Tried {
                endpoint: "deepseek".into(),
                model: "deepseek-v4".into(),
            },
            failed("rate_limited"),
            Push::Model {
                endpoint: "bigmodel".into(),
                model: "glm-5.3-flash".into(),
            },
            Push::ModelChanged {
                endpoint: Some("bigmodel".into()),
                model: Some("glm-5.3-flash".into()),
                limits: Some(Limits {
                    window: Some(200_000),
                    compaction_line: Some(167_000),
                }),
                failover: true,
                reference: Some("@duo".into()),
                effort: None,
            },
        ],
    );
    assert_eq!(
        t.shown_model(),
        Some(("@duo".into(), String::new())),
        "用的是池：底栏一直写池名，换到哪个成员看那一行（2026-10-07 项目主人定）"
    );
    assert_eq!(
        t.model,
        Some(("glm-5.3-flash".into(), "bigmodel".into())),
        "回答末尾那一行照真用的写"
    );
    assert_eq!(t.limits.window, Some(200_000), "限额照换");
    let notes: Vec<&str> = t
        .entries
        .iter()
        .filter(|e| e.kind == Kind::Note)
        .map(|e| e.text.as_str())
        .collect();
    assert_eq!(
        notes,
        [
            "↻ 换了模型：deepseek/deepseek-v4 → bigmodel/glm-5.3-flash",
            "被限速了"
        ]
    );
}

#[test]
fn a_pool_stays_the_pool_in_the_footer_while_its_members_take_turns() {
    // 「配置与模型」第 8 条（2026-10-07 项目主人定）：轮换到哪个成员，回答末尾已经写了，底栏不跟着变。
    let texts = Config::builtin().unwrap().text;
    let mut t = Transcript::default();
    t.update(
        Update::CurrentModel(Current {
            endpoint: None,
            model: None,
            reference: "@daily".into(),
            effort: None,
        }),
        &texts,
    );
    let pool = Some(("@daily".into(), String::new()));
    assert_eq!(t.shown_model(), pool, "订阅回应：轮换的池只有引用");
    apply(
        &mut t,
        vec![
            Push::Model {
                endpoint: "dev".into(),
                model: "m1".into(),
            },
            Push::ModelChanged {
                endpoint: Some("relay".into()),
                model: Some("m2".into()),
                limits: Some(Limits {
                    window: Some(64_000),
                    compaction_line: None,
                }),
                failover: false,
                reference: Some("@daily".into()),
                effort: Some("high".into()),
            },
        ],
    );
    assert_eq!(t.shown_model(), pool, "答话的成员、下一轮的成员都不换底栏");
    assert_eq!(
        t.model,
        Some(("m2".into(), "relay".into())),
        "回答末尾那一行照这一轮真用的写（2026-10-07 实测：写成了 /@daily）"
    );
    assert_eq!(t.limits.window, Some(64_000), "限额照这一轮的成员换");
    assert_eq!(t.effort(), Some("high"), "思考强度照推来的");
    t.update(Update::Configured("dev/m1".into()), &texts);
    apply(
        &mut t,
        vec![Push::Model {
            endpoint: "dev".into(),
            model: "m1".into(),
        }],
    );
    assert_eq!(
        t.shown_model(),
        Some(("m1".into(), "dev".into())),
        "换回单个模型：照推来的写"
    );
}

#[test]
fn a_change_that_is_not_a_failover_writes_nothing() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![Push::ModelChanged {
            endpoint: Some("bigmodel".into()),
            model: Some("glm-5.3-flash".into()),
            limits: None,
            failover: false,
            reference: Some("bigmodel/glm-5.3-flash".into()),
            effort: None,
        }],
    );
    assert!(t.entries.is_empty());
    assert_eq!(t.model, Some(("glm-5.3-flash".into(), "bigmodel".into())));
}

#[test]
fn cooling_is_noted_until_a_call_goes_through() {
    let mut t = Transcript::default();
    apply(&mut t, vec![failed("cooling")]);
    assert_eq!(t.cooling(), Some(None), "都在冷却，还不知道几时恢复");
    let until: Timestamp = "2026-10-01T08:02:00Z".parse().unwrap();
    t.cooling_until(Some(until));
    assert_eq!(t.cooling(), Some(Some(until)));
    apply(&mut t, vec![Push::CallOk]);
    assert_eq!(t.cooling(), None, "又有一次请求成了");
}

#[test]
fn the_reference_in_use_follows_subscribe_turns_and_switches() {
    // 「配置与模型」第 1、8 条，核心 8-10：`/model` 照它标「当前」。
    let texts = Config::builtin().unwrap().text;
    let mut t = Transcript::default();
    t.update(
        Update::CurrentModel(Current {
            endpoint: Some("dev".into()),
            model: Some("m1".into()),
            reference: "dev/m1".into(),
            effort: None,
        }),
        &texts,
    );
    assert_eq!(t.model_ref(), Some("dev/m1"));
    assert_eq!(
        t.model,
        Some(("m1".into(), "dev".into())),
        "开会话时底栏照订阅回应"
    );
    t.update(Update::Configured("@duo".into()), &texts);
    assert_eq!(t.model_ref(), Some("@duo"), "换成了：当场记下");
    assert_eq!(
        t.model,
        Some(("@duo".into(), String::new())),
        "底栏当场写成选的那个"
    );
    assert!(t.entries.is_empty(), "自己换的正文里不写");
}

#[test]
fn a_pinned_model_that_went_away_says_so() {
    // 核心 8-10：钉着的没了，内核退回默认，`replaced` 是原来的。
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![Push::ModelSet {
            reference: "dev/m1".into(),
            replaced: Some("old/m0".into()),
        }],
    );
    assert_eq!(t.model_ref(), Some("dev/m1"));
    let notes: Vec<&str> = t.entries.iter().map(|e| e.text.as_str()).collect();
    assert_eq!(notes, ["↻ 原来的模型没了，换回默认：old/m0 → dev/m1"]);
    apply(
        &mut t,
        vec![Push::ModelSet {
            reference: "dev/m2".into(),
            replaced: None,
        }],
    );
    assert_eq!(t.entries.len(), 1, "别的头换的不写");
}

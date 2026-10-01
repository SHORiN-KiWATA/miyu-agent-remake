//! 子会话用哪个模型（施工 8-8，`models.md`「怎么走」第三条第 4 条）：她写了挡位的，照父会话这一轮的配置解析这一挡（没配的
//! 是 `models.chat`）；没写的，用父会话这时生效的引用。解析出来的交给会话表，记进子会话的 `session.created`。

use super::*;
use support::routing::configs;

/// 一家 `a`，`models.chat` 是 `a/main`，`lite` 挡是池 `@free`，别的挡没配。
const CONFIG: &str = "[providers.a]\nkeys = []\n\n[models]\nchat = \"a/main\"\n\n[models.tiers]\nlite = \"@free\"\n\n[pools.free]\nmodels = [\"a/x\", \"a/y\"]\n";

/// 调一次 `subagent`，带挡位 `tier`。
fn tiered(title: &str, tier: &str) -> (&'static str, String) {
    let args = serde_json::json!({"description": title, "prompt": "Task.", "tier": tier});
    ("subagent", args.to_string())
}

/// 照 `CONFIG` 造一个能派子代理的主会话，会话记着的模型是 `model`（没有的照 `models.chat`）。
async fn parent_with(
    home: &mut Home,
    script: &Script,
    table: &Arc<Table>,
    model: Option<&str>,
) -> Handle {
    home.configs = configs(CONFIG, &[]);
    let lines = Lines {
        sessions: Some(Arc::clone(table) as Arc<dyn SessionPort>),
        model: model.map(str::to_string),
        ..Lines::default()
    };
    home.create_full(script, &basesystem(home), Opening::default(), lines)
        .await
}

/// 交给会话表的子会话，照造的先后，各记着哪个模型。
fn models(table: &Table) -> Vec<Option<String>> {
    let mut made = table.made();
    made.sort_by(|a, b| a.command.as_str().cmp(b.command.as_str()));
    made.into_iter().map(|child| child.model).collect()
}

#[tokio::test]
async fn a_tier_resolves_against_the_parent_config_and_none_inherits_the_parent() {
    let mut home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        calls(&[
            subagent("没写挡位", "Task."),
            tiered("轻量", "lite"),
            tiered("旗舰", "flagship"),
        ]),
        Play::Says("派出去了。"),
    ]);
    let handle = parent_with(&mut home, &script, &table, None).await;
    one_turn(&home, &handle, 1).await;
    let some = |text: &str| Some(text.to_string());
    assert_eq!(
        models(&table),
        [some("a/main"), some("@free"), some("a/main")],
        "没写的抄父会话的（它照造的时候的 chat）、lite 是池、flagship 没配用 chat"
    );
}

#[tokio::test]
async fn without_a_tier_the_child_takes_what_the_parent_was_given() {
    let mut home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        calls(&[subagent("跟父会话", "Task.")]),
        Play::Says("派出去了。"),
    ]);
    let handle = parent_with(&mut home, &script, &table, Some("@free")).await;
    one_turn(&home, &handle, 1).await;
    assert_eq!(models(&table), [Some("@free".to_string())]);
    let log = home.log(handle.id());
    match &log[0].body {
        Body::SessionCreated(created) => {
            assert_eq!(created.model.as_deref(), Some("@free"), "父会话自己记着它");
        }
        other => panic!("{other:?}"),
    }
}

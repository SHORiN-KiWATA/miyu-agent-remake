//! 子代理照池选模型（施工 8-8 补，`models.md`「工具」，取代 8-8 的挡位）：她写了 `pool` 的，子会话记 `@<池>`；没写的、写以前
//! 的 `tier` 的，用父会话这时生效的引用。能选哪几个池在造会话时照那时的配置拼进工具面：开着开关、有成员的才列；以后配置改了，
//! 这个会话（连同载入以后）一个字节不变，新会话才变；老会话里写新会话才列的池照参数不对。

use std::sync::Arc;

use miyu_config::merge::Resolved;
use miyu_config::secret::{Reference, Secret};
use miyu_kernel::event::ToolStatus;
use miyu_session::{ConfigSource, Configs};

use super::*;
use crate::support::routing::{configs, resolved};

/// 一家 `a`，`models.chat` 是 `a/main`。池 `fast` 开着、带说明，`slow` 关着，`off` 没写开关，`lite` 开着、没成员。
const CONFIG: &str = "[providers.a]\nlocal = false\n\n[models]\nchat = \"a/main\"\n\n\
[pools.fast]\nmodels = [\"a/x\"]\nsubagent = true\ndescription = \"Quick lookups.\"\n\n\
[pools.slow]\nmodels = [\"a/y\"]\n\n[pools.off]\nmodels = [\"a/z\"]\n\n[pools.lite]\nmodels = []\nsubagent = true\n";

/// 同一份，后来改了：`slow` 打开了、带说明，`fast` 的说明换了。
const CHANGED: &str = "[providers.a]\nlocal = false\n\n[models]\nchat = \"a/main\"\n\n\
[pools.fast]\nmodels = [\"a/x\"]\nsubagent = true\ndescription = \"Changed.\"\n\n\
[pools.slow]\nmodels = [\"a/y\"]\nsubagent = true\ndescription = \"Big model, slow.\"\n\n\
[pools.off]\nmodels = [\"a/z\"]\n\n[pools.lite]\nmodels = []\nsubagent = true\n";

/// 造会话时照 `CONFIG` 拼出来的 `pool` 那一格。
const FAST_ONLY: &str = r#""pool":{"type":"string","enum":["fast"],"description":"Model pool for the task. Default: your own model.\nfast: Quick lookups."}"#;

/// 照 `CHANGED` 拼出来的。
const BOTH: &str = r#""pool":{"type":"string","enum":["fast","slow"],"description":"Model pool for the task. Default: your own model.\nfast: Changed.\nslow: Big model, slow."}"#;

/// 调一次 `subagent`，带池 `pool`。
fn pooled(title: &str, pool: &str) -> (&'static str, String) {
    let args = serde_json::json!({"description": title, "prompt": "Task.", "pool": pool});
    ("subagent", args.to_string())
}

/// 调一次 `subagent`，带以前的挡位 `tier`。
fn tiered(title: &str, tier: &str) -> (&'static str, String) {
    let args = serde_json::json!({"description": title, "prompt": "Task.", "tier": tier});
    ("subagent", args.to_string())
}

/// 照配置 `configs` 造一个能派子代理的主会话，会话记着的模型是 `model`（没有的照 `models.chat`）。
async fn parent_on(
    home: &mut Home,
    configs: Configs,
    script: &Script,
    table: &Arc<Table>,
    model: Option<&str>,
) -> Handle {
    home.configs = configs;
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

/// 请求里 `subagent` 的参数格式，原样的字。
fn subagent_parameters(request: &Request) -> String {
    request
        .tools
        .iter()
        .find(|tool| tool.name == "subagent")
        .map(|tool| tool.parameters.get().to_string())
        .expect("工具面上有 subagent")
}

/// 一份能换的配置：测试里换掉它，以后造的会话、以后的回合照新的。
#[derive(Debug)]
struct Plain(Resolved);

impl ConfigSource for Plain {
    fn with_project(&self, _: &str) -> Resolved {
        self.0.clone()
    }

    fn secret(&self, _: &Reference) -> Option<Secret> {
        None
    }
}

#[tokio::test]
async fn a_pool_is_recorded_and_no_pool_or_a_tier_takes_the_parent_model() {
    let mut home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        calls(&[
            subagent("没写池", "Task."),
            pooled("快", "fast"),
            tiered("以前的", "lite"),
            pooled("没开的", "slow"),
            pooled("空的", "lite"),
        ]),
        Play::Says("派出去了。"),
    ]);
    let handle = parent_on(&mut home, configs(CONFIG, &[]), &script, &table, None).await;
    let log = one_turn(&home, &handle, 1).await;
    let some = |text: &str| Some(text.to_string());
    assert_eq!(
        models(&table),
        [some("a/main"), some("@fast"), some("a/main")],
        "没写的、写 tier 的抄父会话的（它照造的时候的 chat），写了池的记 @池"
    );
    // 几个调用并行跑，结果在日志里的先后不定：照调用的第几个排好再看。
    let mut done = results(&log);
    done.sort_by_key(|result| (result.call_id.message().get(), result.call_id.index()));
    for (at, pool) in [(3, "slow"), (4, "lite")] {
        assert_eq!(done[at].status, ToolStatus::Error, "{pool}");
        assert_eq!(
            text(done[at]),
            format!("The arguments are not right: unknown variant `{pool}`, expected `fast`.\n"),
            "没开开关的、没成员的不列"
        );
    }
    let request = &script.requests()[0].1;
    assert!(
        subagent_parameters(request).contains(FAST_ONLY),
        "{}",
        subagent_parameters(request)
    );
}

#[tokio::test]
async fn without_a_pool_the_child_takes_what_the_parent_was_given() {
    let mut home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([
        calls(&[subagent("跟父会话", "Task.")]),
        Play::Says("派出去了。"),
    ]);
    let handle = parent_on(
        &mut home,
        configs(CONFIG, &[]),
        &script,
        &table,
        Some("@fast"),
    )
    .await;
    one_turn(&home, &handle, 1).await;
    assert_eq!(models(&table), [Some("@fast".to_string())]);
    let log = home.log(handle.id());
    match &log[0].body {
        Body::SessionCreated(created) => {
            assert_eq!(created.model.as_deref(), Some("@fast"), "父会话自己记着它");
        }
        other => panic!("{other:?}"),
    }
}

/// 造会话时照那时的配置拼：以后配置改了，这个会话的工具面一个字节不变（连同载入以后），新会话照新的；老会话里写新会话才列
/// 的池照参数不对，列着的照样派得出去。
#[tokio::test]
async fn the_face_is_built_when_the_session_starts_and_kept() {
    let mut home = Home::new();
    let table = Arc::new(Table::default());
    let (swap, configs) =
        tokio::sync::watch::channel(Arc::new(Plain(resolved(CONFIG))) as Arc<dyn ConfigSource>);
    let script = Script::new([
        calls(&[pooled("甲", "slow")]),
        Play::Says("好。"),
        calls(&[pooled("乙", "slow")]),
        Play::Says("好。"),
        calls(&[pooled("丙", "fast"), pooled("丁", "slow")]),
        Play::Says("好。"),
    ]);
    let old = parent_on(&mut home, configs, &script, &table, None).await;
    one_turn(&home, &old, 1).await;
    swap.send(Arc::new(Plain(resolved(CHANGED))))
        .expect("还有人在收");
    let log = one_turn(&home, &old, 2).await;
    let statuses: Vec<ToolStatus> = results(&log)
        .iter()
        .map(|result| result.status.clone())
        .collect();
    assert_eq!(
        statuses,
        [ToolStatus::Error, ToolStatus::Error],
        "开局时没列 slow"
    );
    assert!(table.made().is_empty());

    // 新会话照改了的配置拼。
    let fresh_script = Script::new([Play::Says("好。")]);
    let fresh = parent_on(
        &mut home,
        swap.subscribe(),
        &fresh_script,
        &Arc::new(Table::default()),
        None,
    )
    .await;
    one_turn(&home, &fresh, 1).await;
    let fresh_face = subagent_parameters(&fresh_script.requests()[0].1);
    assert!(fresh_face.contains(BOTH), "{fresh_face}");

    // 老会话停了再载入：照快照，不重拼。
    let id = old.id().clone();
    stop(&old).await;
    let port = Some(Arc::clone(&table) as Arc<dyn SessionPort>);
    let loaded = home
        .load_full(&id, &script, &basesystem(&home), &environment().cwd, port)
        .await;
    let log = one_turn(&home, &loaded, 3).await;
    let mut done = results(&log);
    done.sort_by_key(|result| (result.call_id.message().get(), result.call_id.index()));
    assert_eq!(text(done[2]), "Started subagent j1: \"丙\".\n");
    assert_eq!(
        text(done[3]),
        "The arguments are not right: unknown variant `slow`, expected `fast`.\n",
        "载入以后照快照列着的查，不照现在的配置"
    );
    assert_eq!(models(&table), [Some("@fast".to_string())]);
    let faces: Vec<String> = script
        .requests()
        .iter()
        .map(|(_, request)| subagent_parameters(request))
        .collect();
    assert_eq!(faces.len(), 6);
    assert!(faces[0].contains(FAST_ONLY), "{}", faces[0]);
    assert!(
        faces.iter().all(|face| *face == faces[0]),
        "配置改了、载入以后，这个会话的 subagent 一个字节不变"
    );
}

/// 一个池都没列的：参数格式里没有 `pool`，和施工 8-8 以前一样。
#[tokio::test]
async fn without_pools_to_offer_there_is_no_pool_parameter() {
    let mut home = Home::new();
    let table = Arc::new(Table::default());
    let script = Script::new([Play::Says("好。")]);
    let handle = parent_on(
        &mut home,
        configs(
            "[providers.a]\nlocal = false\n\n[pools.lite]\nmodels = []\nsubagent = true\n",
            &[],
        ),
        &script,
        &table,
        None,
    )
    .await;
    one_turn(&home, &handle, 1).await;
    assert_eq!(
        subagent_parameters(&script.requests()[0].1),
        r#"{"type":"object","properties":{"description":{"type":"string","description":"A short title for the task, 3 to 5 words."},"prompt":{"type":"string","description":"The task for the subagent to perform."},"persona":{"type":"string","enum":["engineer"],"description":"Persona for the subagent. Default: no persona.\nengineer"}},"required":["description","prompt"]}"#,
        "没有池的拿掉 pool；人格照出厂的两个填（施工 P-2 补）"
    );
}

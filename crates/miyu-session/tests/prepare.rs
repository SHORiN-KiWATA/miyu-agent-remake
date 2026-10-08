//! 提前压好的开关（施工 6-11 上，`docs/blueprint/compaction.md` 第十五条第 6 条）：配置 `compaction.prepare = false` 的，过了起压线
//! 也不提前压，到线照现在的办法当场压。开着的另见 `prepare_log.rs`。

use miyu_kernel::event::{Body, Purpose};
use miyu_session::testkit::{Play, Script};

use crate::support::routing::configs;
use crate::support::{Home, ask, say, stop, until_turn_ends, watch};

#[tokio::test]
async fn switched_off_in_the_config_it_compacts_at_the_line() {
    let mut home = Home::new();
    home.configs = configs("[compaction]\nprepare = false\n", &[]);
    // 数同 `prepare_log.rs`：第二轮过了起压线，第三轮过了线。
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("嗯。"),
        Play::Says("<summary>当场压的摘要</summary>"),
        Play::Says("行。"),
    ])
    .window(113_000)
    .reports(77_000);
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    for (id, words) in [
        ("cmd-1", "a".repeat(70_000)),
        ("cmd-2", "hi".to_string()),
        ("cmd-3", "b".repeat(20_000)),
    ] {
        ask(&handle, id, say(&words)).await.expect("会话在跑");
        until_turn_ends(&mut pushes).await;
    }
    stop(&handle).await;
    let log = home.log(handle.id());
    assert!(
        !log.iter().any(|event| matches!(&event.body,
            Body::ModelCalled(called) if called.purpose == Some(Purpose::Compaction))),
        "关着的不提前压"
    );
    let summaries: Vec<&str> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted.summary.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(summaries, ["当场压的摘要"]);
}

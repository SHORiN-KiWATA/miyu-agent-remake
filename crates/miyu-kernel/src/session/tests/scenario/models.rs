//! 场景：模型从配置里来以后内核多认的几样（`docs/blueprint/models.md`「事件」、「怎么走」第一条第 7 条，施工 8-6）。
//! 端口当场说完的 `no_model`：这一轮以出错结束，不再来，不推重试的状态。`failover`、`cooling` 随 8-9。

use super::*;
use crate::event::{Body, Status, TransientBody};

/// 推过的重试状态。
fn statuses(stage: &Stage) -> Vec<&Status> {
    stage
        .transients()
        .iter()
        .filter_map(|transient| match &transient.body {
            TransientBody::Status(status) => Some(status),
            _ => None,
        })
        .collect()
}

#[test]
fn no_model_ends_the_turn_without_retrying() {
    let mut stage = stage();
    stage.model([Line::fails(
        ErrorClass::NoModel,
        "no model configured: set models.chat",
    )]);
    stage.say("hi");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 model.called:error kernel t3",
            "7 turn.ended:error kernel t3"
        ])
    );
    assert!(statuses(&stage).is_empty(), "不再来，不推重试");
    assert_eq!(stage.requests().len(), 1);
    let class = stage.log().iter().find_map(|event| match &event.body {
        Body::ModelCalled(called) => called.error.as_ref().map(|error| error.class.as_str()),
        _ => None,
    });
    assert_eq!(class, Some("no_model"), "照这个名字写进日志");
}

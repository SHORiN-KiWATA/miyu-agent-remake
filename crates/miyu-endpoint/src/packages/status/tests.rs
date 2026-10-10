//! 扩展的每一步写成列表上的状态（施工 F-6 上，`package-pages.md`「`package.list` 每一项多的几格」第 3 条）。

use super::of_state;
use crate::extensions::{Reason, State};

#[test]
fn every_extension_state_has_a_status() {
    let later = tokio::time::Instant::now();
    assert_eq!(of_state(State::Off), "off");
    assert_eq!(of_state(State::Starting { pid: None }), "starting");
    assert_eq!(of_state(State::Starting { pid: Some(7) }), "starting");
    assert_eq!(
        of_state(State::Waiting { until: later }),
        "starting",
        "退避中的算启动中"
    );
    assert_eq!(of_state(State::Running { pid: 7 }), "running");
    assert_eq!(of_state(State::Stopped(Reason::NotInstalled)), "stopped");
}

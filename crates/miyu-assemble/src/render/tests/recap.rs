//! 回顾（施工 3-8 四补，`docs/blueprint/kernel/request.md`「组装」）：回顾的 `model.called`、`session.recapped` 不进上下文，也
//! 不改别的怎么排。它们不带回合编号，可以落在回合开始的那几块中间：那一轮开始时注入的事实照样挪到触发前面，前缀接得上。

use super::*;

/// 记下一次回顾：带 `purpose` 的 `model.called` 和那一句，都不带回合编号，照到第 `upto` 条。
pub(super) fn recapped(log: &mut Log, upto: u64) {
    log.detached(
        KERNEL,
        "model.called",
        &format!(r#"{{"seen":{upto},"messages":1,"result":"ok","purpose":"recap"}}"#),
    );
    log.detached(
        KERNEL,
        "session.recapped",
        &format!(r#"{{"text":"在打招呼。","upto":{upto}}}"#),
    );
}

#[test]
fn a_recap_is_not_rendered_and_moves_nothing() {
    let mut log = Log::new();
    let hi = log.say("hi");
    log.start(hi);
    recapped(&mut log, hi);
    log.fact("<env/>");
    assert_eq!(
        rendered(&log),
        ["user: <env/> | hi"],
        "开始时注入的事实照样排在触发前面，回顾的两条不出"
    );
}

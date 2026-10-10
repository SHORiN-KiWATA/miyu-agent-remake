//! 场景：到线了不停（施工 6-11 补，2026-10-10 项目主人定：压缩不等）。数同 `prepare.rs`：窗口 420、输出预留 10、余量 10，
//! 压缩线 400，放得下的上限 410；第三轮的请求 405 过了线、还放得下：照常发主请求，不推进度；压好了放着，下一次发请求前
//! 换上。放不下的照旧等（`prepare_wait.rs`）。

use super::prepare::{compactions, done, preparing, two_turns, words};
use super::*;
use crate::event::TransientBody;

/// 日志一条一行。
fn lines_of(stage: &Stage) -> String {
    story(stage)
        .iter()
        .map(|line| format!("{line}\n"))
        .collect()
}

/// 推过几次进度。
fn progressed(stage: &Stage) -> usize {
    stage
        .transients()
        .iter()
        .filter(|transient| matches!(transient.body, TransientBody::CompactionProgress(_)))
        .count()
}

#[test]
fn one_on_its_way_at_the_line_does_not_stop_the_turn() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1-summary").held()]);
    two_turns(&mut stage, 330, 390);
    // 第三轮说完报 405：下一轮的请求还在线上。
    stage.model([Line::says("好").reports(405)]);
    let requests = stage.requests().len();
    stage.say(&words(10));
    assert_eq!(stage.requests().len(), requests + 1, "照常发了主请求");
    assert_eq!(progressed(&stage), 0, "不停、不推进度");
    assert!(compactions(&stage).is_empty());
    assert!(
        lines_of(&stage).contains("turn.ended:completed"),
        "这一轮照常说完：{}",
        lines_of(&stage)
    );
    // 摘要回来了：没有在等的回合，先放着。
    stage.release_prepare();
    assert!(compactions(&stage).is_empty(), "回来了不当场写");
    // 下一次发请求前到线，换上。
    stage.model([Line::says("又好")]);
    stage.say(&words(2));
    let compacted = compactions(&stage);
    assert_eq!(compacted.len(), 1, "{}", lines_of(&stage));
    assert_eq!(
        (compacted[0].upto.get(), compacted[0].summary.as_str()),
        (8, "P1-summary")
    );
    assert_eq!(done(&stage), [true]);
    assert_eq!(progressed(&stage), 0, "换上的不推进度");
}

#[test]
fn nothing_on_its_way_at_the_line_starts_one_and_goes_on() {
    // G 是 0：起压线就是压缩线，到线时手里什么都没有。
    let mut stage = preparing(0);
    stage.prepare_model([Line::says("P1").held()]);
    two_turns(&mut stage, 330, 390);
    assert!(stage.prepares().is_empty(), "没到线不起");
    stage.model([Line::says("好")]);
    let requests = stage.requests().len();
    stage.say(&words(10));
    assert_eq!(stage.requests().len(), requests + 1, "照常发了主请求");
    assert_eq!(stage.prepares().len(), 1, "到线这时在后台起压");
    assert!(compactions(&stage).is_empty());
    assert_eq!(progressed(&stage), 0);
}

/// 到线以后接着说，尾巴长过了 T + G（100），还在 T + G 加余量（110）以内：压好的那份照用，不扔掉重压。
#[test]
fn a_tail_grown_after_the_line_still_takes_the_prepared_one() {
    let mut stage = preparing(60);
    stage.prepare_model([Line::says("P1").held()]);
    stage.model([
        Line::says(&words(50)).reports(330),
        Line::says(&words(80)).reports(310),
        Line::says("好").reports(405),
        Line::says("又好"),
    ]);
    stage.say(&words(50));
    stage.say(&words(10));
    stage.say(&words(10));
    assert_eq!(progressed(&stage), 0, "第三轮照发：{}", lines_of(&stage));
    stage.release_prepare();
    stage.say(&words(2));
    let compacted = compactions(&stage);
    assert_eq!(compacted.len(), 1, "{}", lines_of(&stage));
    assert_eq!(compacted[0].summary, "P1", "照用压好的那份");
    assert_eq!(done(&stage), [true]);
}

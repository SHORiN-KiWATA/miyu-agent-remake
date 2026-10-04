//! 别处来的话（蓝图 `tui.md`「别处来的话」）：带来处画进正文，子会话开头先画上的交代不画第二遍，撤销不带走它，
//! 不算你说的话开的那一轮；你自己的话落盘时序号不配给它。

use super::apply;
use crate::core::{EndReason, Push};
use crate::transcript::{Kind, Transcript};

fn users(t: &Transcript) -> Vec<(Option<&str>, &str, bool)> {
    t.entries
        .iter()
        .filter(|e| e.kind == Kind::User)
        .map(|e| (e.from.as_deref(), e.text.as_str(), e.hidden))
        .collect()
}

#[test]
fn foreign_lines_carry_where_they_came_from_and_stay_through_an_undo() {
    let mut t = Transcript::default();
    t.foreign(Some(3), "claude-code".into(), "看看测试为什么红了".into());
    apply(
        &mut t,
        vec![
            Push::TurnStarted(4, Some(3)),
            Push::TurnEnded(EndReason::Completed),
        ],
    );
    assert_eq!(
        users(&t),
        [(Some("claude-code"), "看看测试为什么红了", false)]
    );
    assert!(t.last_said().is_none(), "别处来的话开的那一轮不能重做");
    apply(&mut t, vec![Push::Reverted(vec![4])]);
    assert!(!users(&t)[0].2, "撤销不带走它");
    // 你自己的话落盘：序号配给你说的那句，不配给别处来的。
    t.user("我也看看".into(), Vec::new());
    apply(&mut t, vec![Push::UserMessage(8)]);
    let mine = t.entries.iter().find(|e| e.text == "我也看看").unwrap();
    assert_eq!(mine.seq, Some(8));
}

#[test]
fn the_task_drawn_first_in_a_child_is_not_drawn_twice() {
    // 切进子会话：开头先画上交代的活（订阅时可能已经错过了），核心推来的同一句只补上序号。
    let mut t = Transcript::default();
    t.foreign(None, "主会话".into(), "查一下文档".into());
    t.foreign(Some(2), "主会话".into(), "查一下文档".into());
    assert_eq!(users(&t).len(), 1);
    assert_eq!(t.entries[0].seq, Some(2));
    t.foreign(Some(5), "主会话".into(), "再查一处".into());
    assert_eq!(users(&t).len(), 2, "后来的留言照画");
}

#[test]
fn the_output_of_an_ended_command_fills_its_notice() {
    // 施工 7-4 补的 `job.output`：命令结束的那一行，输出读回来以后点开看（「后台命令、子代理和侧边栏」第 5 条）。
    use crate::transcript::JobMark;
    let mut t = Transcript::default();
    t.job_from(
        JobMark::Done,
        "后台命令完成 · ls".into(),
        String::new(),
        Some("j3".into()),
    );
    t.job(JobMark::Done, "别的".into(), String::new());
    t.fill_job("j3", "a.txt".into());
    t.fill_job("j9", "没有这条".into());
    let details: Vec<&str> = t
        .entries
        .iter()
        .map(|e| e.job.as_ref().unwrap().detail.as_str())
        .collect();
    assert_eq!(details, ["a.txt", ""]);
}

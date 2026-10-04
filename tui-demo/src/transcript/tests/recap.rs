//! 回顾（蓝图 `tui.md`「回顾」）：推来的、交回上一句的都画成一段；回顾请求的用量只算进累计。
//! 同是核心的辅助请求起的：自动起的标题，去掉了读成没有。

use super::super::{Kind, Transcript};
use super::apply;
use crate::config::Config;
use crate::core::{EndReason, Push, Update, Usage};

#[test]
fn a_recap_is_a_block_at_the_end_and_its_call_only_adds_to_the_total() {
    let texts = Config::builtin().unwrap().text;
    let mut t = Transcript::default();
    t.update(Update::Push(Push::Recapped("在接回顾。".into())), &texts);
    let last = t.entries.last().unwrap();
    assert_eq!(last.kind, Kind::Recap);
    assert_eq!(last.text, "回顾：在接回顾。");
    t.update(Update::Recap("上一句。".into()), &texts);
    assert_eq!(
        t.entries.last().unwrap().text,
        "回顾：上一句。",
        "交回上一句的照回应画"
    );
    let before = t.context;
    let spent = Usage {
        uncached: 300,
        output: 55,
        ..Usage::default()
    };
    t.update(Update::Push(Push::AuxUsage(spent)), &texts);
    // 回顾单独发、不走缓存：只记进辅助那一格，Σ 算它，命中率、输入输出照主对话算（实测过：原来 C84% 掉到 C76%）。
    assert_eq!(t.total.aux, 355);
    assert_eq!((t.total.uncached, t.total.output), (0, 0));
    assert_eq!(t.context, before, "不改上下文");
}

#[test]
fn a_recap_goes_away_with_the_turn_it_covers_and_comes_back_with_it() {
    // 2026-10-01 项目主人报：撤销以后旧的回顾还在，讲的是撤掉了的那一轮。回顾跟着它讲到的最后一轮走。
    let texts = Config::builtin().unwrap().text;
    let mut t = Transcript::default();
    for turn in [1, 2] {
        t.user(format!("第{turn}句"), Vec::new());
        apply(
            &mut t,
            vec![
                Push::TurnStarted(turn, Some(turn)),
                Push::TurnEnded(EndReason::Completed),
            ],
        );
    }
    t.update(Update::Push(Push::Recapped("讲到第二轮。".into())), &texts);
    let recap = |t: &Transcript| {
        t.entries
            .iter()
            .find(|e| e.kind == Kind::Recap)
            .unwrap()
            .hidden
    };
    apply(&mut t, vec![Push::Reverted(vec![2])]);
    assert!(recap(&t), "撤了它讲到的那一轮：藏起来");
    apply(&mut t, vec![Push::Unreverted(vec![2])]);
    assert!(!recap(&t), "恢复了：再露出来");
    apply(&mut t, vec![Push::Reverted(vec![3])]);
    assert!(!recap(&t), "撤的是它后面的：不动");
}

#[test]
fn a_removed_title_reads_as_untitled() {
    // 2026-10-01 核心 3-8 三补：去掉标题推的是 `"title":""`，读成没有标题（侧边栏写「未命名」），不是空名字。
    let mut t = Transcript::default();
    apply(&mut t, vec![Push::Title("回文函数".into())]);
    assert_eq!(t.title.as_deref(), Some("回文函数"));
    apply(&mut t, vec![Push::Title(String::new())]);
    assert_eq!(t.title, None);
}

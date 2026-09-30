//! 连核心（蓝图 `tui.md`「连核心」第 7、8 条）：断了当场给那一轮收尾、自动重连；连不上写原因。

use super::apply;
use crate::config::Config;
use crate::core::{Block, EndReason, Push, Update};
use crate::transcript::{Kind, Link, Transcript};

#[test]
fn a_turn_cut_off_by_the_core_ends_red_and_the_late_end_adds_nothing() {
    // 2026-09-30 实测：核心被杀，正文停在半句，没有收尾行，之后发的话没人接。
    let texts = Config::builtin().unwrap().text;
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            Push::TurnStarted(1, None),
            Push::BlockStart {
                index: 0,
                block: Block::Text,
            },
            Push::Delta {
                index: 0,
                text: "像谁把一".into(),
            },
        ],
    );
    t.update(Update::Disconnected, &texts);
    let last = t.entries.last().unwrap();
    // 2026-09-30 项目主人：和被打断的「▣ 已中断」一个样子，换个颜色。
    assert_eq!(
        (last.kind.clone(), last.text.as_str(), last.level),
        (
            Kind::Cut,
            "核心断开连接",
            Some(crate::core::Level::Workspace)
        )
    );
    assert!(!t.busy(), "不再转圈、不再走表");
    let count = t.entries.len();
    // 重连以后核心推来那一轮的结束（载入时记成中止）：界面已经收过尾了，不再写。
    apply(
        &mut t,
        vec![Push::TurnEnded(EndReason::Other("aborted".into()))],
    );
    assert_eq!(t.entries.len(), count);
}

#[test]
fn the_link_says_what_happened() {
    let texts = Config::builtin().unwrap().text;
    let mut t = Transcript::default();
    t.update(Update::Missing("/nonexistent/miyu".into()), &texts);
    assert_eq!(
        t.link,
        Link::Down("找不到核心程序：/nonexistent/miyu".into())
    );
    t.update(Update::Ready("s1".into()), &texts);
    t.update(Update::Disconnected, &texts);
    assert_eq!(
        t.link,
        Link::Reconnecting,
        "没在回答时断了：只说在重连，不写正文"
    );
    assert!(t.entries.is_empty());
    t.update(Update::Reconnected, &texts);
    assert_eq!(
        (t.link.clone(), t.session.as_deref()),
        (Link::Ready, Some("s1"))
    );
}

#[test]
fn a_new_session_clears_the_page_but_keeps_the_link_and_the_model() {
    let texts = Config::builtin().unwrap().text;
    let mut t = Transcript::default();
    t.update(Update::Ready("s1".into()), &texts);
    t.user("你好".into(), Vec::new());
    t.note(Kind::Note, "旁白".into());
    t.level = crate::core::Level::Full;
    t.model = Some(("flash".into(), "dev".into()));
    let last_id = t.entries.last().unwrap().id;
    t.split_off();
    assert!(t.entries.is_empty() && t.session.is_none());
    assert_eq!(t.level, crate::core::Level::Workspace, "新会话从工作区开始");
    assert_eq!(
        (t.link.clone(), t.model.clone()),
        (Link::Ready, Some(("flash".into(), "dev".into())))
    );
    t.note(Kind::Note, "新的".into());
    assert!(
        t.entries[0].id > last_id,
        "条目编号接着往上数：排好的行按编号缓存"
    );
}

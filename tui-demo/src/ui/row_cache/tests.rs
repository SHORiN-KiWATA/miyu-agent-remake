//! 正文按条缓存排好的行（蓝图「正文」第 8 条）：和整份重排一模一样；没变的条目不重排，变了的、在进行的才排；
//! 条目按编号认，不按位置；图做好了全部重排。

use std::cell::RefCell;
use std::time::{Duration, Instant};

use serde_json::json;

use super::{RowCache, build};
use crate::core::ToolStatus;
use crate::transcript::{JobMark, Kind, Segment, Step, StepKind, ToolState, Transcript};
use crate::ui::rows::Target;
use crate::ui::test_support::{Fixture, fresh_rows};

fn finished_segment(t0: Instant) -> Segment {
    let mut thought = Step::new(StepKind::Thought {
        text: "先看看目录".into(),
    });
    thought.started = t0;
    thought.took = Some(Duration::from_secs(1));
    let mut shell = Step::new(StepKind::Tool {
        name: "shell".into(),
        args: String::new(),
        parsed: json!({"command": "ls", "description": "列目录"}),
        state: ToolState::Done(ToolStatus::Ok),
        output: "a.txt".into(),
        said: None,
    });
    shell.started = t0;
    shell.took = Some(Duration::from_secs(1));
    let mut seg = Segment::new();
    seg.steps = vec![thought, shell];
    seg.finished = true;
    seg
}

/// 四条：你说的话、一段时间线、回答、一条后台通知。
fn sample() -> Transcript {
    let mut t = Transcript::default();
    t.note(Kind::User, "你好".into());
    t.note(Kind::Steps, String::new());
    t.entries.last_mut().unwrap().segment = Some(finished_segment(Instant::now()));
    t.note(Kind::Reply, "## 标题\n\n- 一\n- 二\n\n一段话。".into());
    t.job(JobMark::Done, "后台命令完成 · ls".into(), "a.txt".into());
    t
}

fn lines(rows: impl Iterator<Item = String>) -> Vec<String> {
    rows.collect()
}

#[test]
fn cached_rows_match_a_fresh_build() {
    let f = Fixture::new();
    let t = sample();
    let ctx = f.ctx();
    let cache = RefCell::new(RowCache::default());
    let fresh = fresh_rows(&t.entries, &ctx);
    for _ in 0..2 {
        let cached = build(&t.entries, &ctx, &cache);
        assert_eq!(cached.len(), fresh.len());
        assert_eq!(
            lines(cached.iter().map(|r| r.line.to_string())),
            lines(fresh.iter().map(|r| r.line.to_string()))
        );
        let targets: Vec<Option<Target>> = cached.iter().map(|r| r.target).collect();
        assert_eq!(targets, fresh.iter().map(|r| r.target).collect::<Vec<_>>());
        assert_eq!(
            cached.get(fresh.len() - 1).unwrap().plain,
            fresh.last().unwrap().plain
        );
    }
}

#[test]
fn only_changed_entries_are_rebuilt() {
    // 数重排了几条：主题别在中途被别的测试换掉。
    let _theme = crate::theme::hold();
    let f = Fixture::new();
    let mut t = sample();
    let mut ctx = f.ctx();
    let cache = RefCell::new(RowCache::default());
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 4, "头一帧全排");
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 0, "什么都没变：一条都不排");
    t.entries[3].open = true;
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 1, "点开了一条：只排它");
    ctx.hover = Some(Target::Segment(1));
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 1, "悬停在一条上：只排它");
    ctx.hover = Some(Target::Entry(3));
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 2, "悬停挪了：离开的和进来的");
    t.entries[2].text.push_str("\n\n又一段。");
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 1, "回答长了：只排它");
    ctx.width = 40;
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 4, "宽度变了：全排");
}

#[test]
fn a_live_segment_is_rebuilt_every_frame() {
    // 数重排了几条：主题别在中途被别的测试换掉。
    let _theme = crate::theme::hold();
    let f = Fixture::new();
    let mut t = sample();
    t.entries[1]
        .segment
        .as_mut()
        .unwrap()
        .steps
        .push(Step::new(StepKind::Thought {
            text: "还在想".into(),
        }));
    t.entries[1].segment.as_mut().unwrap().finished = false;
    let ctx = f.ctx();
    let cache = RefCell::new(RowCache::default());
    build(&t.entries, &ctx, &cache);
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 1, "转圈、走表的那一段每帧重排");
}

#[test]
fn entries_are_known_by_id_not_position() {
    let f = Fixture::new();
    let mut t = sample();
    let ctx = f.ctx();
    let cache = RefCell::new(RowCache::default());
    build(&t.entries, &ctx, &cache);
    // 排队退回的消息会从中间抽走：后面的条目挪了位置，点中的东西要跟着对。
    t.entries.remove(0);
    let cached = build(&t.entries, &ctx, &cache);
    let fresh = fresh_rows(&t.entries, &ctx);
    let targets: Vec<Option<Target>> = cached.iter().map(|r| r.target).collect();
    assert_eq!(targets, fresh.iter().map(|r| r.target).collect::<Vec<_>>());
}

#[test]
fn a_finished_figure_rebuilds_everything() {
    // 数重排了几条：主题别在中途被别的测试换掉。
    let _theme = crate::theme::hold();
    let f = Fixture::new();
    let t = sample();
    let ctx = f.ctx();
    let cache = RefCell::new(RowCache::default());
    build(&t.entries, &ctx, &cache);
    // 图做好了、被扔掉了：占几行变了，全排一遍。
    ctx.figures.borrow_mut().forget();
    build(&t.entries, &ctx, &cache);
    assert_eq!(cache.borrow().rebuilt, 4);
}

#[test]
fn a_pasted_block_in_what_you_said_is_replaced_by_its_full_text() {
    // 你说的话里的粘贴块：品红，点它（点中这一条）在下面铺底色展开全文（`tui.md`「正文」第 2 条）。
    let f = Fixture::new();
    let mut t = Transcript::default();
    let full = "报错第一行\n报错第二行".to_string();
    t.user(
        "看看：[已粘贴 2 行]".into(),
        vec![("[已粘贴 2 行]".into(), full)],
    );
    let ctx = f.ctx();
    let rows = fresh_rows(&t.entries, &ctx);
    let label = rows
        .iter()
        .flat_map(|r| r.line.spans.iter())
        .find(|s| s.content == "[已粘贴 2 行]")
        .expect("块单独一截");
    assert_eq!(label.style, crate::theme::chip(), "和输入框里一样的小块");
    assert!(
        rows.iter().any(|r| r.target == Some(Target::Entry(0))),
        "能点"
    );
    assert!(!rows.iter().any(|r| r.plain.contains("报错第一行")), "收着");
    t.entries[0].open = true;
    let open = fresh_rows(&t.entries, &ctx);
    let text: Vec<&str> = open.iter().map(|r| r.plain.as_str()).collect();
    // 原地替换：块换成全文，接在「看看：」后面；不铺底色。
    assert!(
        !text.iter().any(|l| l.contains("[已粘贴")),
        "块换掉了：{text:?}"
    );
    assert!(text.contains(&"看看：报错第一行"), "{text:?}");
    assert!(text.contains(&"报错第二行"), "{text:?}");
    assert!(!open.iter().any(|r| r.shade), "不铺底色");
    assert!(
        open.iter().all(|r| r.target == Some(Target::Entry(0))),
        "哪一行都能点回去"
    );
}

#[test]
fn hovering_what_you_said_lifts_the_blocks() {
    // 悬停：块的底色亮一档；展开着的，粘进来的那几段铺上块的底色（`tui.md`「正文」第 2 条）。
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.user(
        "看看：[已粘贴 2 行]".into(),
        vec![("[已粘贴 2 行]".into(), "报错第一行\n报错第二行".into())],
    );
    let mut ctx = f.ctx();
    ctx.hover = Some(Target::Entry(0));
    let rows = fresh_rows(&t.entries, &ctx);
    let label = rows
        .iter()
        .flat_map(|r| r.line.spans.iter())
        .find(|s| s.content == "[已粘贴 2 行]")
        .unwrap();
    assert_eq!(label.style, crate::theme::chip_hover());
    assert_ne!(crate::theme::chip_hover(), crate::theme::chip());
    t.entries[0].open = true;
    let open = fresh_rows(&t.entries, &ctx);
    let pasted = open
        .iter()
        .flat_map(|r| r.line.spans.iter())
        .find(|s| s.content == "报错第二行")
        .unwrap();
    assert_eq!(pasted.style.bg, crate::theme::chip().bg, "粘的那段铺底色");
    let said = open
        .iter()
        .flat_map(|r| r.line.spans.iter())
        .find(|s| s.content == "看看：")
        .unwrap();
    assert_eq!(said.style.bg, None, "自己打的字不铺");
}

#[test]
fn same_looking_blocks_in_what_you_said_open_to_their_own_text() {
    // 两块写出来一样：点开各换成各自的原文（照先后），不是两处都换成第一块的。
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.user(
        "[已粘贴 1 行]和[已粘贴 1 行]".into(),
        vec![
            ("[已粘贴 1 行]".into(), "甲".into()),
            ("[已粘贴 1 行]".into(), "乙".into()),
        ],
    );
    t.entries[0].open = true;
    let ctx = f.ctx();
    let rows = fresh_rows(&t.entries, &ctx);
    assert!(
        rows.iter().any(|r| r.plain == "甲和乙"),
        "{:?}",
        rows.iter().map(|r| r.plain.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn a_details_title_opens_and_closes_its_reply() {
    // 回答里的 `<details>`：标题那一行能点，点过的记在这一条上，重排照它开关（蓝图「她的回答：Markdown」第 15 条）。
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.note(
        Kind::Reply,
        "<details>\n<summary>点我展开</summary>\n\n里面的字。\n\n</details>".into(),
    );
    let cache = RefCell::new(RowCache::default());
    let plain = |t: &Transcript, ctx: &crate::ui::rows::Ctx| -> Vec<String> {
        build(&t.entries, ctx, &cache)
            .iter()
            .map(|r| r.plain.clone())
            .collect()
    };
    let target = Target::Details(0, 0);
    let shut = build(&t.entries, &f.ctx(), &cache);
    let title = shut.iter().find(|r| r.target == Some(target)).unwrap();
    assert_eq!(title.plain, "点我展开", "标题那一行能点，记号不复制");
    assert!(!plain(&t, &f.ctx()).iter().any(|l| l.contains("里面的字")));
    // 点一下：记在这一条上，缓存着的也重排。
    t.entries[0].details.push(0);
    assert!(plain(&t, &f.ctx()).iter().any(|l| l.contains("里面的字")));
    // 悬停：标题变亮。
    let mut ctx = f.ctx();
    ctx.hover = Some(target);
    let lit = build(&t.entries, &ctx, &cache);
    let row = lit.iter().find(|r| r.target == Some(target)).unwrap();
    assert!(
        row.line
            .spans
            .iter()
            .any(|s| s.style.fg == crate::theme::hover().fg),
        "悬停变亮"
    );
}

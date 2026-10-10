//! 按页读：最上面那一行、更早的一页拼在前面、撤销跨页、订阅回应的累计（蓝图「会话列表」第 5 条「按页读」）。

use crate::config::{Config, Texts};
use crate::core::{Bill, Block, EndReason, Level, Push, Spent, Update, Usage};
use crate::transcript::{Kind, Transcript};

fn texts() -> Texts {
    Config::builtin().unwrap().text
}

fn apply(t: &mut Transcript, pushes: Vec<Push>) {
    let texts = texts();
    for p in pushes {
        t.update(Update::Push(p), &texts);
    }
}

/// 第 `n` 轮：你说 `said`（序号 `n * 10`），她答 `reply`。
fn turn(n: u64, said: &str, reply: &str) -> Vec<Push> {
    vec![
        Push::Said {
            seq: n * 10,
            text: said.into(),
        },
        Push::TurnStarted(n, Some(n * 10)),
        Push::BlockStart {
            index: 0,
            block: Block::Text,
        },
        Push::Delta {
            index: 0,
            text: reply.into(),
        },
        Push::BlockEnd(0),
        Push::TurnEnded(EndReason::Completed),
    ]
}

/// 正文里的字（不算收尾行）。
fn words(t: &Transcript) -> Vec<String> {
    t.entries
        .iter()
        .filter(|e| !e.hidden && e.kind != Kind::Done)
        .map(|e| e.text.clone())
        .collect()
}

#[test]
fn the_top_line_comes_and_goes_with_more() {
    let mut t = Transcript::default();
    apply(&mut t, turn(3, "第 3 句", "三"));
    t.paged(true, &texts());
    assert_eq!(t.entries[0].kind, Kind::Older);
    assert_eq!(t.entries[0].text, "正在加载更早的内容…");
    assert!(t.wants_older());
    t.asked_older();
    assert!(!t.wants_older(), "要了还没回来，不再要");
    t.paged(true, &texts());
    assert_eq!(
        t.entries.iter().filter(|e| e.kind == Kind::Older).count(),
        1
    );
    t.paged(false, &texts());
    assert!(t.entries.iter().all(|e| e.kind != Kind::Older));
    assert!(!t.wants_older());
}

#[test]
fn an_earlier_page_goes_in_front_with_fresh_ids_and_the_marker_follows_more() {
    let texts = texts();
    let mut t = Transcript::default();
    apply(&mut t, turn(3, "第 3 句", "三"));
    t.paged(true, &texts);
    let mut older = t.history();
    apply(&mut older, turn(1, "第 1 句", "一"));
    apply(&mut older, turn(2, "第 2 句", "二"));
    let n = t.prepend(older, true, &texts);
    assert!(n > 0);
    assert_eq!(
        t.entries[0].kind,
        Kind::Older,
        "还有更早的：那一行还在最上面"
    );
    assert_eq!(
        words(&t)[1..],
        ["第 1 句", "一", "第 2 句", "二", "第 3 句", "三"]
    );
    let mut ids: Vec<u64> = t.entries.iter().map(|e| e.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), t.entries.len(), "编号不撞");
    // 接着来的照旧接在后面。
    apply(&mut t, turn(4, "第 4 句", "四"));
    assert_eq!(words(&t).last().map(String::as_str), Some("四"));
    // 最早的一页：那一行去掉。
    let older = t.history();
    t.prepend(older, false, &texts);
    assert!(t.entries.iter().all(|e| e.kind != Kind::Older));
}

#[test]
fn undo_in_a_newer_page_hides_turns_that_arrive_later() {
    let texts = texts();
    let mut t = Transcript::default();
    // 新的页里撤了第 1、2 轮，又恢复了第 2 轮；第 1、2 轮在更早的页里。
    apply(&mut t, turn(3, "第 3 句", "三"));
    apply(
        &mut t,
        vec![Push::Reverted(vec![1, 2]), Push::Unreverted(vec![2])],
    );
    let mut older = t.history();
    apply(&mut older, turn(1, "第 1 句", "一"));
    apply(&mut older, turn(2, "第 2 句", "二"));
    // 更早的页里自己撤过第 2 轮：新的页恢复了，照新的。
    apply(&mut older, vec![Push::Reverted(vec![2])]);
    t.prepend(older, false, &texts);
    assert_eq!(words(&t), ["第 2 句", "二", "第 3 句", "三"]);
}

#[test]
fn the_subscribe_reply_replaces_what_the_page_added_up() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![Push::Usage(Usage {
            uncached: 6,
            cache_read: 4,
            cache_write: 0,
            output: 1,
            aux: 0,
        })],
    );
    let total = Usage {
        uncached: 600,
        cache_read: 400,
        cache_write: 0,
        output: 100,
        aux: 60,
    };
    let spent = Spent {
        total,
        bill: Bill::default(),
        compactions: 2,
        breaks: 1,
    };
    t.snapshot(Some(&spent), Some(Level::Full));
    assert_eq!(t.total, total);
    assert_eq!((t.cache.compactions, t.cache.breaks), (2, 1));
    assert_eq!(t.level, Level::Full);
    // 之后照推来的往上加；上下文照最近那次请求。
    apply(
        &mut t,
        vec![Push::Usage(Usage {
            uncached: 10,
            cache_read: 0,
            cache_write: 0,
            output: 5,
            aux: 0,
        })],
    );
    assert_eq!(t.total.output, 105);
    assert_eq!(t.context, 15);
    // 核心旧、没有的那一样不动。
    t.snapshot(None, None);
    assert_eq!(t.total.output, 105);
    assert_eq!(t.level, Level::Full);
}

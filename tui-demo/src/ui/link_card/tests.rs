//! 链接卡片的排版与交互回归。

use crate::core::Card;
use crate::transcript::{Kind, Transcript};
use crate::ui::test_support::Fixture;

fn text(rows: &[crate::ui::rows::Row]) -> Vec<String> {
    rows.iter()
        .map(|r| r.line.to_string().trim().to_string())
        .collect()
}

#[test]
fn a_link_alone_on_its_line_becomes_a_card_and_an_inline_one_stays() {
    // 2026-10-02 项目主人定：只给独占一行的链接做，就地换成卡片；回答写完了才换。
    let f = Fixture::new();
    f.cards.borrow_mut().got_card(
        "https://a.dev/x".into(),
        Some(Card {
            title: "A 的文章".into(),
            description: "讲点什么".into(),
            site: "a.dev".into(),
            image: None,
            icon: None,
            ..Card::default()
        }),
    );
    let mut t = Transcript::default();
    t.note(
        Kind::Reply,
        "看这个：\nhttps://a.dev/x\n还有 https://a.dev/x 也行".into(),
    );
    let rows = crate::ui::rows::entry_rows(0, &t.entries[0], &f.ctx());
    let shown = text(&rows);
    assert!(shown.iter().any(|l| l == "A 的文章"), "{shown:#?}");
    assert!(shown.iter().any(|l| l == "a.dev"), "{shown:#?}");
    assert!(shown.iter().any(|l| l == "讲点什么"), "{shown:#?}");
    assert!(
        !shown.iter().any(|l| l == "https://a.dev/x"),
        "那一行换掉了：{shown:#?}"
    );
    assert!(
        shown
            .iter()
            .any(|l| l.contains("还有 https://a.dev/x 也行")),
        "夹在字里的不动：{shown:#?}"
    );
    let card = rows
        .iter()
        .find(|r| r.line.to_string().contains("A 的文章"))
        .unwrap();
    assert!(
        card.links
            .iter()
            .any(|(_, _, url)| url == "https://a.dev/x"),
        "点卡片是点这个链接"
    );
    // 她还在写这一条：不换。
    let mut ctx = f.ctx();
    ctx.writing = Some(t.entries[0].id);
    let shown = text(&crate::ui::rows::entry_rows(0, &t.entries[0], &ctx));
    assert!(shown.iter().any(|l| l == "https://a.dev/x"), "{shown:#?}");
}

#[test]
fn a_card_has_a_blank_line_around_it_skips_empty_lines_and_links_only_its_text() {
    // 2026-10-02 项目主人报：卡片上下没有空行；没有简介的两行也排成三行；悬停时整行（连空的地方）画下划线。
    let f = Fixture::new();
    f.cards.borrow_mut().got_card(
        "https://w.org/rust".into(),
        Some(Card {
            title: "Rust".into(),
            description: String::new(),
            site: "w.org".into(),
            image: None,
            icon: None,
            ..Card::default()
        }),
    );
    let mut t = Transcript::default();
    t.note(Kind::Reply, "介绍：\nhttps://w.org/rust\n就这些。".into());
    let rows = crate::ui::rows::entry_rows(0, &t.entries[0], &f.ctx());
    let shown = text(&rows);
    assert_eq!(
        shown,
        ["介绍：", "", "Rust", "w.org", "", "就这些。"],
        "上下各空一行、没有简介的只两行"
    );
    let title = &rows[2];
    assert_eq!(title.links.len(), 1);
    let (from, to, _) = &title.links[0];
    assert_eq!((*from, *to), (0, 4), "只有字那一截能点：{:?}", title.links);
}

#[test]
fn a_bare_address_alone_in_what_you_said_becomes_a_card_too() {
    let f = Fixture::new();
    f.cards.borrow_mut().got_card(
        "https://a.dev/x".into(),
        Some(Card {
            title: "A 的文章".into(),
            description: String::new(),
            site: "a.dev".into(),
            image: None,
            icon: None,
            ..Card::default()
        }),
    );
    let mut t = Transcript::default();
    t.user("帮我看看\nhttps://a.dev/x".into(), Vec::new());
    let rows = crate::ui::rows::entry_rows(0, &t.entries[0], &f.ctx());
    let shown = text(&rows);
    assert!(shown.iter().any(|l| l.ends_with("A 的文章")), "{shown:#?}");
    assert!(
        !shown.iter().any(|l| l.ends_with("https://a.dev/x")),
        "{shown:#?}"
    );
    let card = rows
        .iter()
        .find(|r| r.line.to_string().contains("A 的文章"))
        .unwrap();
    assert!(
        card.line.to_string().contains('┃'),
        "竖线照样在：{}",
        card.line
    );
}

#[test]
fn a_link_without_a_card_is_asked_for_once_and_stays_a_link() {
    let f = Fixture::new();
    let mut t = Transcript::default();
    t.note(Kind::Reply, "https://b.dev".into());
    let shown = text(&crate::ui::rows::entry_rows(0, &t.entries[0], &f.ctx()));
    assert!(shown.iter().any(|l| l == "https://b.dev"), "{shown:#?}");
    let (cards, _) = f.cards.borrow_mut().take();
    assert_eq!(cards, ["https://b.dev"], "记进单子");
}

//! 网站卡片：真图片解码、视频脚注、文章快照和窄窗口。
use super::rows;
use crate::{
    core::{Card, CardKind},
    figures::{Figures, Graphics},
    ui::test_support::Fixture,
};
use image::{Rgb, RgbImage};
use ratatui::text::Span;
use ratatui_image::picker::{Picker, ProtocolType};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

struct Cover(PathBuf);
impl Cover {
    fn new(f: &Fixture) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "miyu-link-cover-{}-{}.png",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        RgbImage::from_pixel(160, 90, Rgb([30, 60, 90]))
            .save(&path)
            .unwrap();
        f.cards
            .borrow_mut()
            .saved("cover".into(), Some(path.clone()));
        Self(path)
    }
}
impl Drop for Cover {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).unwrap();
    }
}

fn card(kind: CardKind) -> Card {
    Card {
        kind,
        title: "昨日海 PV".into(),
        site: "哔哩哔哩".into(),
        author: Some("明日方舟".into()),
        duration: Some(408),
        description: "活动宣传片".into(),
        image: Some("cover".into()),
        ..Card::default()
    }
}

fn rendered(f: &Fixture, card: &Card, width: u16) -> Vec<crate::ui::rows::Row> {
    let (tx, rx) = mpsc::channel();
    let mut picker = Picker::halfblocks();
    picker.set_protocol_type(ProtocolType::Kitty);
    *f.ctx().figures.borrow_mut() = Figures::start(
        Some(Graphics { picker }),
        &f.config.figures,
        None,
        move |done| tx.send(done).is_ok(),
    );
    let mut ctx = f.ctx();
    ctx.width = width;
    let pending = rows(&Span::raw(""), &[], "https://video.test", card, &ctx);
    assert!(
        pending.iter().all(|r| !r.line.to_string().contains('▶')),
        "尚未显示封面不出脚注"
    );
    let done = rx.recv_timeout(Duration::from_secs(5)).expect("图片解码");
    ctx.figures.borrow_mut().done(done);
    rows(&Span::raw(""), &[], "https://video.test", card, &ctx)
}

#[test]
fn video_footer_is_below_the_cover_and_links_to_the_video() {
    let f = Fixture::new();
    let _cover = Cover::new(&f);
    let shown = rendered(&f, &card(CardKind::Video), 60);
    assert!(
        shown
            .iter()
            .any(|r| r.line.to_string().contains("哔哩哔哩 · 明日方舟"))
    );
    let footer = shown.last().unwrap();
    assert_eq!(footer.line.to_string(), "▶ 6:48");
    assert!(footer.figure.is_none());
    assert!(shown[..shown.len() - 1].iter().any(|r| r.figure.is_some()));
    assert_eq!(footer.links, [(0, 6, "https://video.test".into())]);
    assert!(footer.copy);
}

#[test]
fn duration_is_written_as_minutes_or_hours_and_missing_duration_is_optional() {
    let f = Fixture::new();
    let _cover = Cover::new(&f);
    for (duration, expected) in [
        (Some(213), "▶ 3:33"),
        (Some(3723), "▶ 1:02:03"),
        (Some(60), "▶ 1:00"),
        (None, "▶"),
        (Some(0), "▶"),
    ] {
        let mut c = card(CardKind::Video);
        c.duration = duration;
        assert_eq!(
            rendered(&f, &c, 60).last().unwrap().line.to_string(),
            expected
        );
    }
}

#[test]
fn other_kinds_and_videos_without_displayed_covers_have_no_footer() {
    let f = Fixture::new();
    let _cover = Cover::new(&f);
    for kind in [CardKind::Article, CardKind::Page] {
        assert!(
            rendered(&f, &card(kind), 60)
                .iter()
                .all(|r| !r.line.to_string().contains('▶'))
        );
    }
    let plain = Fixture::new();
    for image in [None, Some("cover".into())] {
        let mut c = card(CardKind::Video);
        c.image = image;
        let shown = rows(&Span::raw(""), &[], "https://video.test", &c, &plain.ctx());
        assert!(shown.iter().all(|r| !r.line.to_string().contains('▶')));
    }
}

#[test]
fn site_and_author_skip_empty_parts_and_narrow_cards_stay_inside_the_width() {
    let f = Fixture::new();
    for (site, author, expected) in [
        ("", Some("作者"), Some("作者")),
        ("网站", None, Some("网站")),
        ("", None, None),
    ] {
        let c = Card {
            title: "标题".into(),
            site: site.into(),
            author: author.map(str::to_string),
            ..Card::default()
        };
        let shown = rows(&Span::raw(""), &[], "https://video.test", &c, &f.ctx());
        assert_eq!(shown.len(), if expected.is_some() { 2 } else { 1 });
        if let Some(expected) = expected {
            assert_eq!(shown[1].line.to_string(), expected);
        }
    }
    let _cover = Cover::new(&f);
    for width in [1, 3, 8, 20, 60] {
        let shown = rendered(&f, &card(CardKind::Video), width);
        assert!(
            shown.iter().all(|r| r.line.width() <= usize::from(width)),
            "宽 {width}: {shown:?}"
        );
        assert!(
            shown
                .iter()
                .flat_map(|r| &r.links)
                .all(|(from, to, _)| from < to && *to <= width)
        );
    }
}

/// 图片本身交给 kitty；文字样本用阴影格标出其真实占位，记录左右列和脚注位置。
fn sample(f: &Fixture, c: &Card) -> String {
    let shown = rendered(f, c, 60);
    let mut text = Vec::new();
    for row in shown {
        let line = row.line.to_string();
        if let Some(cell) = row.figure {
            let cols = usize::from(f.ctx().figures.borrow().cols(cell.key).unwrap());
            text.push("░".repeat(cols) + &line.chars().skip(cols).collect::<String>());
        } else {
            text.push(line);
        }
    }
    text.join("\n") + "\n"
}

#[test]
fn video_and_article_layouts_match_the_saved_samples() {
    let f = Fixture::new();
    let _cover = Cover::new(&f);
    let video = sample(&f, &card(CardKind::Video));
    let article = sample(
        &f,
        &Card {
            kind: CardKind::Article,
            title: "Pacman".into(),
            site: "ArchWiki".into(),
            description: "Arch Linux 包管理器".into(),
            image: Some("cover".into()),
            ..Card::default()
        },
    );
    if std::env::var_os("MIYU_UPDATE_CARD_SAMPLES").is_some() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/link_card/samples");
        std::fs::write(dir.join("video.txt"), &video).unwrap();
        std::fs::write(dir.join("article.txt"), &article).unwrap();
    }
    assert_eq!(video, include_str!("samples/video.txt"));
    assert_eq!(article, include_str!("samples/article.txt"));
}

#[test]
fn a_failed_cover_keeps_the_text_without_a_play_footer() {
    let f = Fixture::new();
    let cover = Cover::new(&f);
    std::fs::write(&cover.0, b"not an image").unwrap();
    let shown = rendered(&f, &card(CardKind::Video), 60);
    assert!(
        shown
            .iter()
            .all(|r| r.figure.is_none() && !r.line.to_string().contains('▶'))
    );
    assert!(
        shown
            .iter()
            .any(|r| r.line.to_string() == "哔哩哔哩 · 明日方舟")
    );
}

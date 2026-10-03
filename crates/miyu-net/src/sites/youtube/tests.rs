use super::*;

#[test]
fn iso_durations_become_seconds() {
    for (iso, expected) in [
        ("PT3M33S", Some(213)),
        ("PT1H2M3S", Some(3723)),
        ("PT0M7S", Some(7)),
        ("PT59S", Some(59)),
        ("P1DT2H", Some(93_600)),
        ("P1D", Some(86_400)),
        (" PT10M ", Some(600)),
        // 是 0 的不写
        ("PT0S", None),
        ("P0D", None),
        // 读不了的不写
        ("", None),
        ("P", None),
        ("PT", None),
        ("3M33S", None),
        ("PT3.5S", None),
        ("PTM", None),
        ("PT3S3M", None),
        ("PT3M3M", None),
        ("PT3X", None),
        ("PT33", None),
        ("P1H", None),
        ("PT99999999999999999999S", None),
    ] {
        assert_eq!(seconds(iso), expected, "{iso:?}");
    }
}

#[test]
fn a_page_with_a_duration_is_a_video_and_one_without_is_a_page() {
    let watch = r#"<meta property="og:title" content="V"><meta itemprop="duration" content="PT3M33S">
        <span itemprop="author"><link itemprop="url" href="/@r"><link itemprop="name" content="Rick"></span>"#;
    let mut draft = Draft::default();
    fill(watch, &mut draft);
    assert_eq!(draft.kind, Kind::Video);
    assert_eq!(draft.duration, Some(213));
    assert_eq!(draft.author, "Rick");
    let channel = r#"<meta property="og:title" content="Channel">"#;
    let mut draft = Draft::default();
    fill(channel, &mut draft);
    assert_eq!(draft.kind, Kind::Page);
    assert_eq!(draft.duration, None);
    assert_eq!(draft.author, "");
}

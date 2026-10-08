//! 什么时候重读场所规则（施工 O-21，`onebot.md` 第一条「场所规则和出厂数据」第 3 条）：系统的规则文件、违规词表改了、加了、
//! 删了，隔够 `rules_check_millis` 的下一次用照新的，没隔够的照旧；这一刻的样子（文件列表、修改时刻、大小）没变的不重读。
//! 钟是交进去的时刻，测试不等。

use std::time::{Duration, Instant};

use miyu_onebot::rules::Venues;

use crate::rules::{FACTORY_WORDS, clean, group, system_rule, system_words, text};
use crate::support::*;

#[test]
fn a_changed_system_file_is_read_at_the_next_use_after_the_interval() {
    let (dir, root) = temp_root();
    let every = Duration::from_secs(1);
    let start = Instant::now();
    let at = |millis| start + Duration::from_millis(millis);
    let mut venues = Venues::new(factory(), &root, every, start);
    let rate = |venues: &mut Venues, millis| {
        venues.current(at(millis)).at(&group("1")).resolved.entries["rate"]
            .value
            .clone()
    };
    assert_eq!(rate(&mut venues, 0), text("5/300s"));
    // 加了一份：没隔够照旧，隔够了照新的。
    let path = system_rule(&root, "80-x.toml", "[[rule]]\nrate = \"30/60s\"\n");
    assert_eq!(rate(&mut venues, 999), text("5/300s"), "没隔够");
    assert_eq!(rate(&mut venues, 1000), text("30/60s"), "隔够了");
    // 改了：从上一次看的时候起算。
    std::fs::write(&path, "[[rule]]\nrate = \"700/600s\"\n").expect("写得进");
    assert_eq!(rate(&mut venues, 1999), text("30/60s"));
    assert_eq!(rate(&mut venues, 2000), text("700/600s"));
    // 删了：回到出厂的。
    std::fs::remove_file(&path).expect("删得了");
    assert_eq!(rate(&mut venues, 3000), text("5/300s"));
    clean(&dir);
}

#[test]
fn an_unchanged_listing_is_not_read_again() {
    let (dir, root) = temp_root();
    let path = system_rule(&root, "80-x.toml", "[[rule]]\nrate = \"30/60s\"\n");
    let start = Instant::now();
    let mut venues = Venues::new(factory(), &root, Duration::from_secs(1), start);
    let modified = std::fs::metadata(&path)
        .and_then(|metadata| metadata.modified())
        .expect("有修改时刻");
    // 一样长、修改时刻改回去：这一刻的样子和上一次的一样，不重读，照手里的。
    std::fs::write(&path, "[[rule]]\nrate = \"40/60s\"\n").expect("写得进");
    std::fs::File::options()
        .write(true)
        .open(&path)
        .and_then(|file| file.set_modified(modified))
        .expect("改得了修改时刻");
    let loaded = venues.current(start + Duration::from_secs(5));
    assert_eq!(
        loaded.at(&group("1")).resolved.entries["rate"].value,
        text("30/60s")
    );
    // 长短变了：重读。
    std::fs::write(&path, "[[rule]]\nrate = \"400/60s\"\n").expect("写得进");
    std::fs::File::options()
        .write(true)
        .open(&path)
        .and_then(|file| file.set_modified(modified))
        .expect("改得了修改时刻");
    let loaded = venues.current(start + Duration::from_secs(10));
    assert_eq!(
        loaded.at(&group("1")).resolved.entries["rate"].value,
        text("400/60s")
    );
    clean(&dir);
}

#[test]
fn a_changed_word_list_is_read_at_the_next_use_too() {
    let (dir, root) = temp_root();
    let start = Instant::now();
    let mut venues = Venues::new(factory(), &root, Duration::from_secs(1), start);
    assert_eq!(venues.current(start).keywords.len(), FACTORY_WORDS);
    let path = system_words(&root, "foo\n");
    assert_eq!(
        venues.current(start + Duration::from_secs(1)).keywords,
        ["foo"]
    );
    std::fs::remove_file(&path).expect("删得了");
    assert_eq!(
        venues
            .current(start + Duration::from_secs(2))
            .keywords
            .len(),
        FACTORY_WORDS
    );
    clean(&dir);
}

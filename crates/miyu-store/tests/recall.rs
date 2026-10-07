//! 检索库（施工 R-1，`docs/blueprint/recall.md`「怎么走」第二条）：建、重开、坏了和版本不对的删掉重建；放进、换掉、拿掉；
//! 两个字的词、一个字都搜得到；中了越多的越靠前；中英混着的；只给几条。

mod support;

use std::fs;
use std::path::PathBuf;

use miyu_store::recall::{Hit, Opened, RecallIndex};

use support::*;

/// 临时数据根里管理员的一个检索库的位置。
fn place(scratch: &Scratch) -> PathBuf {
    root_in(scratch)
        .index(&admin())
        .join("recall")
        .join("test.db")
}

fn keys(hits: &[Hit]) -> Vec<&str> {
    hits.iter().map(|hit| hit.key.as_str()).collect()
}

#[test]
fn a_new_index_is_created_and_kept_on_reopen() {
    let scratch = Scratch::new("recall-reopen");
    let path = place(&scratch);
    let (index, opened) = RecallIndex::open(&path);
    assert!(matches!(opened, Opened::Created), "{opened:?}");
    index.put("a", "她记得你用 N 卡", at(1)).unwrap();
    drop(index);
    let (index, opened) = RecallIndex::open(&path);
    assert!(matches!(opened, Opened::Kept), "{opened:?}");
    assert_eq!(keys(&index.search("N卡", 10).unwrap()), ["a"]);
    assert_eq!(index.keys().unwrap(), ["a"]);
}

#[test]
fn a_broken_or_outdated_file_is_rebuilt_empty() {
    let scratch = Scratch::new("recall-broken");
    let path = place(&scratch);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        &path,
        b"not a database at all, just some bytes that are long enough",
    )
    .unwrap();
    let (index, opened) = RecallIndex::open(&path);
    assert!(matches!(opened, Opened::Rebuilt(_)), "{opened:?}");
    index.put("a", "记忆", at(1)).unwrap();
    assert_eq!(keys(&index.search("记忆", 10).unwrap()), ["a"]);
    drop(index);

    let db = rusqlite::Connection::open(&path).unwrap();
    db.pragma_update(None, "user_version", 99).unwrap();
    drop(db);
    let (index, opened) = RecallIndex::open(&path);
    assert!(matches!(opened, Opened::Rebuilt(_)), "{opened:?}");
    assert!(index.keys().unwrap().is_empty());
}

#[test]
fn replacing_and_removing_change_what_is_found() {
    let scratch = Scratch::new("recall-put");
    let (index, _) = RecallIndex::open(&place(&scratch));
    index.put("a", "今天去看了樱花", at(1)).unwrap();
    index.put("b", "明天要考试", at(2)).unwrap();
    assert_eq!(keys(&index.search("樱花", 10).unwrap()), ["a"]);

    index.put("a", "今天去爬山了", at(3)).unwrap();
    assert!(index.search("樱花", 10).unwrap().is_empty());
    assert_eq!(keys(&index.search("爬山", 10).unwrap()), ["a"]);

    index.remove("a").unwrap();
    index.remove("missing").unwrap();
    assert!(index.search("爬山", 10).unwrap().is_empty());
    assert_eq!(index.keys().unwrap(), ["b"]);
}

#[test]
fn two_character_words_and_single_characters_are_found() {
    let scratch = Scratch::new("recall-short");
    let (index, _) = RecallIndex::open(&place(&scratch));
    index.put("cat", "我的猫很可爱", at(1)).unwrap();
    index.put("memory", "她的记忆系统", at(2)).unwrap();
    assert_eq!(keys(&index.search("猫", 10).unwrap()), ["cat"]);
    assert_eq!(keys(&index.search("记忆", 10).unwrap()), ["memory"]);
    assert_eq!(keys(&index.search("東京", 10).unwrap()), Vec::<&str>::new());
}

#[test]
fn more_matched_terms_rank_first_and_ranks_count_from_zero() {
    let scratch = Scratch::new("recall-rank");
    let (index, _) = RecallIndex::open(&place(&scratch));
    index.put("one", "周末打算去爬山", at(1)).unwrap();
    index
        .put("both", "周末打算去爬山，顺便看樱花", at(2))
        .unwrap();
    index.put("none", "工作日要上班", at(3)).unwrap();
    let hits = index.search("周末去爬山看樱花", 10).unwrap();
    assert_eq!(keys(&hits), ["both", "one"]);
    assert_eq!(hits.iter().map(|hit| hit.rank).collect::<Vec<_>>(), [0, 1]);
}

#[test]
fn chinese_and_english_mixed() {
    let scratch = Scratch::new("recall-mixed");
    let (index, _) = RecallIndex::open(&place(&scratch));
    index.put("rust", "他在用Rust写一个编辑器", at(1)).unwrap();
    index.put("go", "她用 Go 写后端", at(2)).unwrap();
    assert_eq!(keys(&index.search("rust 编辑器", 10).unwrap()), ["rust"]);
    assert_eq!(keys(&index.search("GO", 10).unwrap()), ["go"]);
}

#[test]
fn only_as_many_as_asked_and_nothing_for_an_empty_query() {
    let scratch = Scratch::new("recall-limit");
    let (index, _) = RecallIndex::open(&place(&scratch));
    for n in 0..5 {
        index.put(&format!("k{n}"), "一样的话", at(n)).unwrap();
    }
    assert_eq!(index.search("一样", 3).unwrap().len(), 3);
    assert!(index.search("，。！", 3).unwrap().is_empty());
    assert!(index.search("一样", 0).unwrap().is_empty());
}

/// 量尺（施工单 R-1「验收」第 3 条）：一万条中文短句放进库、找一次各要多久，库多大。只断言结果，不断言耗时。
/// `cargo test -p miyu-store --release --test recall -- --ignored --nocapture`。
#[test]
#[ignore = "量尺"]
fn measure_ten_thousand_sentences() {
    let scratch = Scratch::new("recall-measure");
    let path = place(&scratch);
    let (index, _) = RecallIndex::open(&path);
    let words = [
        "今天",
        "明天",
        "周末",
        "我们",
        "一起",
        "去看",
        "樱花",
        "爬山",
        "写代码",
        "编辑器",
        "考试",
        "显卡",
        "猫咪",
        "吃饭",
        "火锅",
        "电影",
        "游戏",
        "加班",
        "睡觉",
        "下雨",
    ];
    let started = std::time::Instant::now();
    for n in 0..10_000usize {
        let sentence: String = (0..8)
            .map(|k| words[(n * 7 + k * 13 + n / 3) % words.len()])
            .collect();
        index.put(&format!("k{n}"), &sentence, at(0)).unwrap();
    }
    let put = started.elapsed();
    let started = std::time::Instant::now();
    let hits = index.search("周末一起去看樱花吃火锅", 20).unwrap();
    let search = started.elapsed();
    assert_eq!(hits.len(), 20);
    drop(index);
    let size: u64 = ["", "-wal"]
        .iter()
        .map(|suffix| {
            let mut name = path.clone().into_os_string();
            name.push(suffix);
            fs::metadata(name).map(|m| m.len()).unwrap_or(0)
        })
        .sum();
    println!(
        "put 10000: {put:?}, search: {search:?}, size: {} KiB",
        size / 1024
    );
}

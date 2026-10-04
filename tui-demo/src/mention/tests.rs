//! `@` 文件列表的状态（蓝图 `tui.md`「`@` 文件列表」第 1、2 条）：跟着光标前面的词开关，`Esc` 关掉以后词没变就一直关着；
//! 词变了问核心一次，回应照哪个词问的认，清单还在建的隔 `poll_ms` 再问。

use std::time::{Duration, Instant};

use serde_json::json;

use super::{Ask, Mention, Status};
use crate::config::Config;

fn mention() -> Mention {
    Mention::new(Config::builtin().unwrap().mention)
}

#[test]
fn which_question_goes_to_the_core() {
    let list = |dir: &str, prefix: &str| Ask::List {
        dir: dir.into(),
        prefix: prefix.into(),
    };
    assert_eq!(Ask::of("", false), list("", ""), "只打了 @：列这一层");
    assert_eq!(Ask::of("src/ma", true), list("src/", "ma"));
    assert_eq!(Ask::of("~/Doc", false), list("~/", "Doc"));
    assert_eq!(Ask::of("~Doc", false), list("~/", "Doc"));
    assert_eq!(Ask::of("/etc/", false), list("/etc/", ""));
    assert_eq!(
        Ask::of("main", true),
        Ask::Find {
            query: "main".into(),
            fresh: true
        }
    );
}

#[test]
fn a_layer_reply_is_written_the_way_it_was_typed() {
    let reply = json!({"items": [
        {"dir": true, "full": "/h/Documents", "marks": [0, 1, 2], "path": "Documents/"},
        {"dir": false, "full": "/h/Doc.txt", "marks": [0, 1, 2], "path": "Doc.txt", "size": 3},
    ], "partial": false});
    let (items, status, building) = Ask::of("~Doc", false).read(Some(&reply));
    assert_eq!(status, Status::Layer);
    assert!(!building);
    assert_eq!(items[0].shown, "~/Documents/");
    assert_eq!(items[0].path, std::path::PathBuf::from("/h/Documents"));
    assert!(items[0].dir && !items[1].dir);
    assert_eq!(items[0].hits, [2, 3, 4], "对上的字往后挪打的那一截");
    let (none, status, _) = Ask::of("main", false).read(None);
    assert!(none.is_empty(), "回了错的当一条都没有");
    assert_eq!(status, Status::Full);
}

#[test]
fn the_word_asks_once_and_a_building_index_is_asked_again() {
    let mut m = mention();
    let now = Instant::now();
    assert!(m.find("看看", 6).is_none());
    assert!(m.next_ask(now).is_none(), "没开不问");
    let text = "看看 @main";
    let found = m.find(text, text.len()).unwrap();
    assert_eq!((found.start, found.query.as_str()), ("看看 ".len(), "main"));
    assert!(
        found.items.is_empty() && !found.ready,
        "还没回：空着、不算对不上"
    );
    let (word, ask) = m.next_ask(now).unwrap();
    assert_eq!(
        ask,
        Ask::Find {
            query: "main".into(),
            fresh: true
        },
        "刚弹出来写 fresh"
    );
    assert!(m.next_ask(now).is_none(), "问过的词不再问");
    let building = json!({"building": true, "items": [
        {"dir": false, "full": "/w/src/main.rs", "marks": [4, 5, 6, 7], "path": "src/main.rs"}
    ], "partial": false});
    m.replied(&word, Some(&building));
    let found = m.find(text, text.len()).unwrap();
    assert!(found.ready);
    assert_eq!(found.status, Status::Indexing);
    assert_eq!(found.items[0].shown, "src/main.rs");
    assert!(m.next_ask(now).is_none(), "隔 poll_ms 才再问");
    let due = m.deadline().expect("清单在建：到点醒来");
    let (_, again) = m.next_ask(due).unwrap();
    assert_eq!(
        again,
        Ask::Find {
            query: "main".into(),
            fresh: false
        }
    );
    let done = json!({"building": false, "items": [], "partial": true});
    m.replied(&word, Some(&done));
    assert_eq!(m.find(text, text.len()).unwrap().status, Status::Partial);
    assert!(m.deadline().is_none() && m.next_ask(due + Duration::from_secs(1)).is_none());
}

#[test]
fn a_late_reply_for_an_old_word_is_not_used_but_the_last_one_stays_shown() {
    let mut m = mention();
    let now = Instant::now();
    m.find("@ma", 3).unwrap();
    let (old, _) = m.next_ask(now).unwrap();
    let reply = |path: &str| {
        json!({"building": false, "partial": false, "items": [
            {"dir": false, "full": format!("/w/{path}"), "marks": [], "path": path}
        ]})
    };
    m.replied(&old, Some(&reply("main.rs")));
    m.find("@mai", 4).unwrap();
    let (new, ask) = m.next_ask(now).unwrap();
    assert_eq!(
        ask,
        Ask::Find {
            query: "mai".into(),
            fresh: false
        }
    );
    let found = m.find("@mai", 4).unwrap();
    assert!(!found.ready);
    assert_eq!(
        found.items[0].shown, "main.rs",
        "新的没到：先照上一个词的列，不闪成空的"
    );
    m.replied(&old, Some(&reply("stale.rs")));
    assert_eq!(
        m.find("@mai", 4).unwrap().items[0].shown,
        "main.rs",
        "旧词的不用"
    );
    m.replied(&new, Some(&reply("mail.rs")));
    let found = m.find("@mai", 4).unwrap();
    assert!(found.ready);
    assert_eq!(found.items[0].shown, "mail.rs");
}

#[test]
fn esc_keeps_it_shut_until_the_word_changes() {
    let mut m = mention();
    let found = m.find("@main", 5).unwrap();
    m.dismiss(&found);
    assert!(m.find("@main", 5).is_none(), "Esc 关掉，词没变就一直关着");
    assert!(m.next_ask(Instant::now()).is_none(), "关着不问");
    assert!(m.find("@mai", 4).is_some(), "词变了照常开");
}

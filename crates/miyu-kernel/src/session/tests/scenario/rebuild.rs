//! 场景：压后重建（施工 6-5）。第一轮她读了几个文件，第二轮一开头就压：发摘要请求之前先交出重读，候选是被替代的那一段
//! 里读过的，最近的在前、尾巴里读过的跳过；取到摘要以后照先后挑，个数、单个、合计有上限，太大的进清单；代码写的几段
//! 带着清单和取回指路。窗口不够的不重读。

use super::*;
use crate::event::{ContextCompacted, RestoredFile};
use crate::id::ContentHash;
use crate::session::{Compaction, Notes, Rebuild};
use crate::template::Template;

/// 会压缩、会重读的替身：输出预留、余量各 10，尾巴 0；最多重读 2 个，单个 20 token，合计 30，窗口 100 起重读，候选 3 个。
fn rebuilding(tail: u64) -> Stage {
    rebuilding_up_to(tail, 30)
}

/// 同 [`rebuilding`]，合计最多 `total`。
fn rebuilding_up_to(tail: u64, total: u64) -> Stage {
    let make = move || {
        let mut policy = policy();
        policy.compaction = Some(Compaction {
            reserve_cap: 10,
            margin: 10,
            tail,
            price: crate::estimate::Flat {
                image: 50,
                file: 50,
            },
            rebuild: Some(Rebuild {
                files: 2,
                file_tokens: 20,
                total,
                min_window: 100,
                candidates: 3,
            }),
            pause: None,
            shorten: None,
            isolate: false,
        });
        let template = |source: &str| Template::parse(source).unwrap();
        policy.notes = Some(Notes {
            files: template("<files>\n"),
            files_more: template("<more {count}/>\n"),
            retrieve: template("<retrieve {upto}/>\n"),
            too_large: template("<too-large {files}/>\n"),
            uncovered: None,
        });
        policy
    };
    Stage::new(make, environment("/w"), at(0))
}

/// 第一轮一次读四个文件（`/w` 下的 a 到 d，内容照 `texts`），再答一句（报 5000）；交窗口 `window`，第二轮一开头就压。
fn read_then_compact(stage: &mut Stage, texts: [&str; 4], window: u64) {
    read_then_compact_without(stage, texts, window, None);
}

/// 同 [`read_then_compact`]，只是 `missing` 那一个压的时候已经不在「磁盘」上了。
fn read_then_compact_without(
    stage: &mut Stage,
    texts: [&str; 4],
    window: u64,
    missing: Option<&str>,
) {
    let names = ["a.rs", "b.rs", "c.rs", "d.rs"];
    let calls: Vec<(&str, String)> = names
        .iter()
        .map(|name| ("read", format!(r#"{{"path":"{name}"}}"#)))
        .collect();
    let calls: Vec<(&str, &str)> = calls.iter().map(|(n, a)| (*n, a.as_str())).collect();
    stage.model([
        Line::calls("看看。", &calls),
        Line::says("好。").reports(5_000),
    ]);
    stage.tools(
        names
            .iter()
            .zip(texts)
            .map(|(name, text)| Play::read(&format!("/w/{name}"), text)),
    );
    for (name, text) in names.iter().zip(texts) {
        if missing != Some(*name) {
            stage.disk(&format!("/w/{name}"), text);
        }
    }
    stage.say("读一下");
    stage.limits(Some(window), None);
    stage.model([Line::says("S1"), Line::says("嗯。")]);
    stage.say("接着来");
}

/// 写下的那一条压缩。
fn compacted(stage: &Stage) -> &ContextCompacted {
    stage
        .log()
        .iter()
        .find_map(|event| match &event.body {
            Body::ContextCompacted(compacted) => Some(compacted),
            _ => None,
        })
        .expect("压了")
}

#[test]
fn the_newest_files_read_before_the_cut_are_reread_first() {
    let mut stage = rebuilding(0);
    read_then_compact(&mut stage, ["a", "b", "c", "d"], 120);
    let compacted = compacted(&stage);
    // 候选最近的在前、最多 3 个；排在摘要请求前面，名字是同一个。
    let rereads = stage.rereads();
    assert_eq!(rereads.len(), 1);
    let (seen, paths, limit) = &rereads[0];
    assert_eq!(paths, &["/w/d.rs", "/w/c.rs", "/w/b.rs"]);
    assert_eq!(*limit, 80, "单个 20 token 折成 80 字节");
    assert_eq!(*seen, compacted.upto);
    assert!(stage.requests().iter().any(|(named, _)| named == seen));
    // 挑满 2 个为止，照先后；路径照工作目录写成相对的。
    assert_eq!(
        compacted.restored,
        [
            RestoredFile {
                path: "d.rs".to_string(),
                blob: ContentHash::of(b"d"),
                tokens: 1,
            },
            RestoredFile {
                path: "c.rs".to_string(),
                blob: ContentHash::of(b"c"),
                tokens: 1,
            },
        ]
    );
    // 清单是被替代的那一段里读过的全部，最近的在前；取回指路写着替代到哪。
    assert_eq!(
        compacted.notes,
        format!(
            "<files>\n- d.rs\n- c.rs\n- b.rs\n- a.rs\n<retrieve {}/>\n",
            compacted.upto
        )
    );
}

#[test]
fn too_large_and_over_budget_files_are_listed_instead() {
    let mut stage = rebuilding(0);
    // 候选是 d、c、b：d 81 字节，超过执行器的上限；c 80 字节、20 个 token，正好不超过单个；b 16 个 token，加上合计 36，
    // 超过 30。a 不是候选。
    let (a, b, c, d) = (
        "w".repeat(65),
        "z".repeat(64),
        "y".repeat(80),
        "x".repeat(81),
    );
    read_then_compact(&mut stage, [&a, &b, &c, &d], 120);
    let compacted = compacted(&stage);
    let restored: Vec<&str> = compacted
        .restored
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(restored, ["c.rs"]);
    assert_eq!(compacted.restored[0].tokens, 20);
    assert!(
        compacted.notes.ends_with("<too-large d.rs, b.rs/>\n"),
        "{}",
        compacted.notes
    );
}

#[test]
fn files_read_again_in_the_tail_are_not_candidates() {
    // 第一轮读 a、b，第二轮又读 a、读 c；尾巴留下第二轮：a 在尾巴里又读过，c 只在尾巴里，候选只剩 b。
    let mut stage = rebuilding(1_000);
    for (turn, names) in [["a.rs", "b.rs"], ["a.rs", "c.rs"]].into_iter().enumerate() {
        let calls: Vec<String> = names
            .iter()
            .map(|name| format!(r#"{{"path":"{name}"}}"#))
            .collect();
        let calls: Vec<(&str, &str)> = calls.iter().map(|args| ("read", args.as_str())).collect();
        stage.model([
            Line::calls("看看。", &calls),
            Line::says("好。").reports(5_000),
        ]);
        stage.tools(names.map(|name| Play::read(&format!("/w/{name}"), name)));
        for name in names {
            stage.disk(&format!("/w/{name}"), name);
        }
        stage.say(if turn == 0 { "读一下" } else { "再读" });
    }
    // 窗口 140、线 120：尾巴至多 30 个 token，留得下第二轮，留不下第一轮读文件的那一组。
    stage.limits(Some(140), None);
    stage.model([Line::says("S1"), Line::says("嗯。")]);
    stage.say("接着来");
    let reread: Vec<&String> = stage
        .rereads()
        .iter()
        .flat_map(|(_, paths, _)| paths)
        .collect();
    assert_eq!(reread, ["/w/b.rs"]);
}

#[test]
fn a_small_window_rereads_nothing_but_still_writes_the_notes() {
    let mut stage = rebuilding(0);
    read_then_compact(&mut stage, ["a", "b", "c", "d"], 99);
    assert!(stage.rereads().is_empty(), "窗口不到 100 不重读");
    let compacted = compacted(&stage);
    assert!(compacted.restored.is_empty());
    assert!(
        compacted.notes.starts_with("<files>\n- d.rs"),
        "{}",
        compacted.notes
    );
}

#[test]
fn unreadable_files_are_skipped_and_the_next_one_is_taken() {
    let mut stage = rebuilding(0);
    read_then_compact(&mut stage, ["a", "b", "c", "d"], 120);
    assert_eq!(compacted(&stage).restored.len(), 2);
    // 「磁盘」上没有 d 的：读不到的跳过，照先后挑下一个。
    let mut stage = rebuilding(0);
    read_then_compact_without(&mut stage, ["a", "b", "c", "d"], 120, Some("d.rs"));
    let restored: Vec<&str> = compacted(&stage)
        .restored
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(restored, ["c.rs", "b.rs"]);
    assert!(
        !compacted(&stage).notes.contains("too-large"),
        "读不到的不进清单：{}",
        compacted(&stage).notes
    );
}

#[test]
fn the_whole_request_after_compaction_stays_under_half_the_line() {
    // 合计不设限（1000），窗口 110、线 90：压完的整份请求至多 45 个 token。替身的组装压完只剩几条，两个 20 个 token 的
    // 文件只放得下一个。
    let mut stage = rebuilding_up_to(0, 1_000);
    let (a, b) = ("a".repeat(80), "b".repeat(80));
    read_then_compact(&mut stage, [&a, &b, &a, &b], 110);
    let compacted = compacted(&stage);
    assert_eq!(compacted.restored.len(), 1, "{compacted:?}");
    assert!(
        compacted.notes.contains("<too-large"),
        "{}",
        compacted.notes
    );
}

#[test]
fn a_result_that_does_not_match_the_candidates_is_ignored() {
    // 摘要请求在路上时又来一份结果，个数对不上：不收，照先前那一份挑。
    let mut stage = rebuilding(0);
    let names = ["a.rs", "b.rs", "c.rs", "d.rs"];
    let calls: Vec<String> = names
        .iter()
        .map(|name| format!(r#"{{"path":"{name}"}}"#))
        .collect();
    let calls: Vec<(&str, &str)> = calls.iter().map(|args| ("read", args.as_str())).collect();
    stage.model([
        Line::calls("看看。", &calls),
        Line::says("好。").reports(5_000),
    ]);
    stage.tools(names.map(|name| Play::read(&format!("/w/{name}"), name)));
    for name in names {
        stage.disk(&format!("/w/{name}"), name);
    }
    stage.say("读一下");
    stage.limits(Some(120), None);
    stage.model([Line::says("S1").held(), Line::says("嗯。")]);
    stage.say("接着来");
    stage.answer_reread(vec![crate::session::Reread::Unreadable]);
    stage.release_model();
    let restored: Vec<&str> = compacted(&stage)
        .restored
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(restored, ["d.rs", "c.rs"]);
}

/// 检查点换了、原文不在内存里，取回它重读过的文件的原文（施工 6-9）：撤掉压缩回到没有检查点的，不取；恢复了、载入
/// 以后、撤掉后来的一次回到它的，都取回它那几份。
#[test]
fn a_checkpoint_that_counts_again_gets_its_reread_files_recalled() {
    let mut stage = rebuilding(0);
    read_then_compact(&mut stage, ["a", "b", "c", "d"], 120);
    let blobs: Vec<ContentHash> = compacted(&stage)
        .restored
        .iter()
        .map(|file| file.blob.clone())
        .collect();
    assert_eq!(blobs, [ContentHash::of(b"d"), ContentHash::of(b"c")]);
    let compacting = stage.turns()[1];
    stage.revert(compacting);
    assert!(stage.recalls().is_empty(), "撤到没有检查点，不取");
    stage.unrevert();
    assert_eq!(
        stage.recalls(),
        std::slice::from_ref(&blobs),
        "恢复了压缩，取回它的"
    );
    stage.crash();
    assert_eq!(stage.recalls().len(), 2, "载入以后也取回");
    stage.compact("S2");
    assert_eq!(stage.recalls().len(), 2, "新的检查点没重读过文件，不取");
    let later = *stage.turns().last().unwrap();
    stage.revert(later);
    assert_eq!(
        stage.recalls().last(),
        Some(&blobs),
        "撤掉后来的一次，回到它"
    );
    assert_eq!(stage.recalls().len(), 3);
}

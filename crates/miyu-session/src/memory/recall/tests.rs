//! 联想那一块（施工 R-8）：这一段交过的照 `present` 算（模块 `memory` 两类都算，联想带过几条只数 `recall`，别的模块不算）；
//! 一块逐字节对、换行写成空格、日期照时区；超过 500 字节的那一行和以后的不加，一行都放不下的没有。

use std::path::Path;

use miyu_kernel::facts::Present;
use miyu_kernel::id::{FactKind, ModuleId, Seq};
use miyu_kernel::origin::{By, Person};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_recall::associate::Thresholds;
use miyu_recall::{Entry, MemoryId};

use super::{LIMIT, RecallTexts, render, seen};

fn texts() -> RecallTexts {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    RecallTexts::with(&resources, Thresholds::default()).expect("读得到")
}

fn present(module: &str, kind: &str, refs: &[&str]) -> Present {
    Present {
        module: ModuleId::parse(module).unwrap(),
        kind: FactKind::parse(kind).unwrap(),
        refs: refs.iter().map(ToString::to_string).collect(),
    }
}

fn entry(n: u64, text: &str) -> Entry {
    Entry {
        id: MemoryId::new(Seq::new(n).unwrap()),
        class: "user".into(),
        text: text.into(),
        sources: Vec::new(),
        audience: vec![By::Person(Person::new(
            miyu_kernel::id::AccountId::parse("alice").unwrap(),
        ))],
        about: None,
        replaces: None,
        // UTC 的 10 月 7 日 20 点：东九区是 8 日。
        at: Timestamp::parse("2026-10-07T20:00:00.000Z").unwrap(),
        by: By::Kernel,
        replaced_by: None,
        retired: None,
        cleared: false,
    }
}

#[test]
fn what_was_brought_is_counted_from_present() {
    let (ids, brought) = seen(&[
        present("memory", "memory", &["m1", "m2"]),
        present("memory", "recall", &["m3"]),
        present("memory", "recall", &["m4", "m5"]),
        present("roleplay", "recall", &["m9"]),
    ]);
    let ids: Vec<String> = ids.iter().map(ToString::to_string).collect();
    assert_eq!(ids, ["m1", "m2", "m3", "m4", "m5"], "别的模块不算");
    assert_eq!(brought, 3, "只数联想带过的");
    assert_eq!(seen(&[]).1, 0);
}

#[test]
fn a_block_is_exact_and_stays_within_the_limit() {
    let east9 = UtcOffset::from_minutes(540).unwrap();
    let (a, b) = (entry(3, "用户养了一只猫\n叫团子"), entry(7, "回答要短"));
    let block = render(&texts(), &[&a, &b], east9).expect("有");
    assert_eq!(block.module.as_str(), "memory");
    assert_eq!(block.fact.kind.as_str(), "recall");
    assert_eq!(
        block.fact.text,
        "<recalled>\nm3 user 2026-10-08: 用户养了一只猫 叫团子\nm7 user 2026-10-08: 回答要短\n</recalled>\n"
    );
    assert_eq!(block.fact.refs, ["m3", "m7"]);

    let long = entry(4, &"长".repeat(140));
    let block = render(&texts(), &[&a, &long, &b], east9).expect("有");
    assert!(block.fact.text.len() <= LIMIT, "{}", block.fact.text.len());
    assert_eq!(block.fact.refs, ["m3"], "放不下的那一行和以后的都不加");

    let huge = entry(5, &"长".repeat(200));
    assert!(
        render(&texts(), &[&huge], east9).is_none(),
        "一行都放不下的没有"
    );
    assert!(render(&texts(), &[], east9).is_none());
}

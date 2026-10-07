//! 声明本身（`chat.md` 第八条「怎么走」第 1 条）：每个名字正好一个 `.`、不重名；五张表照先后、同一张表的挨在一起；
//! 只有 `judge.model` 可以不写，只有 `chatty.restraint_k` 只收正数。

use std::collections::BTreeSet;

use super::{ITEMS, find, in_table, names, table, tables};

#[test]
fn every_key_is_one_table_and_one_name_and_none_repeats() {
    let mut seen = BTreeSet::new();
    for item in ITEMS {
        assert_eq!(item.key.matches('.').count(), 1, "{}", item.key);
        assert!(
            !item.table().is_empty() && !item.name().is_empty(),
            "{}",
            item.key
        );
        assert!(seen.insert(item.key), "{} 重名", item.key);
        assert!(find(item.key).is_some_and(|found| found.key == item.key));
        assert!(in_table(item.table(), item.name()).is_some_and(|found| found.key == item.key));
    }
}

#[test]
fn the_five_tables_come_in_order_and_each_is_one_block() {
    // 同一张表的不挨在一起，`tables` 去重以后会多出来。
    assert_eq!(
        tables(),
        ["inbound", "chatty", "dispatch", "judge", "outbound"]
    );
    assert_eq!(table("chatty"), Some("chatty"));
    assert_eq!(table("chaty"), None);
    assert_eq!(table("chatty.base"), None);
    assert!(names("dispatch").eq(["supersede_window"]));
}

#[test]
fn only_the_judge_model_is_optional_and_only_restraint_k_is_positive() {
    let optional: Vec<_> = ITEMS
        .iter()
        .filter(|item| item.optional)
        .map(|item| item.key)
        .collect();
    assert_eq!(optional, ["judge.model"]);
    let positive: Vec<_> = ITEMS
        .iter()
        .filter(|item| item.positive)
        .map(|item| item.key)
        .collect();
    assert_eq!(positive, ["chatty.restraint_k"]);
}

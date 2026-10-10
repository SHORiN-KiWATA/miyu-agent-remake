//! 一条记忆印成一行（`docs/blueprint/cli/memory.md`「样子」）：编号照最长的对齐、日期照时区换天、作废的带原因。

use serde_json::json;

use miyu_kernel::time::UtcOffset;

use super::shown::listed;
use crate::language::Language;

fn memory(id: &str, at: &str, text: &str, retired: Option<&str>) -> serde_json::Value {
    json!({"id": id, "at": at, "text": text, "retired": retired, "by": "person", "class": "user", "sources": []})
}

#[test]
fn one_line_each_ids_aligned_and_forgotten_ones_marked() {
    let memories = [
        memory("m12", "2026-10-08T03:00:00.000Z", "用户住在上海", None),
        memory("m8", "2026-10-07T03:00:00.000Z", "回答要短，先说结论", None),
        memory(
            "m3",
            "2026-10-07T02:00:00.000Z",
            "用户养了两只猫",
            Some("试一下"),
        ),
        memory("m2", "2026-10-07T01:00:00.000Z", "用户用 A 卡", Some("")),
    ];
    assert_eq!(
        listed(&memories, UtcOffset::UTC, &Language::Chinese),
        [
            "m12  2026-10-08  用户住在上海",
            "m8   2026-10-07  回答要短，先说结论",
            "m3   2026-10-07  用户养了两只猫（已作废：试一下）",
            "m2   2026-10-07  用户用 A 卡（已作废）",
        ]
    );
    assert_eq!(
        listed(&memories[2..3], UtcOffset::UTC, &Language::English),
        ["m3  2026-10-07  用户养了两只猫 (forgotten: 试一下)"]
    );
}

#[test]
fn the_day_follows_the_time_zone() {
    let late = [memory("m1", "2026-10-07T20:30:00.000Z", "晚上记的", None)];
    let tokyo = UtcOffset::from_minutes(9 * 60).expect("合范围");
    let new_york = UtcOffset::from_minutes(-4 * 60).expect("合范围");
    assert_eq!(
        listed(&late, tokyo, &Language::Chinese),
        ["m1  2026-10-08  晚上记的"]
    );
    assert_eq!(
        listed(&late, new_york, &Language::Chinese),
        ["m1  2026-10-07  晚上记的"]
    );
}

#[test]
fn nothing_says_so() {
    assert_eq!(Language::Chinese.no_memories(), "还没有记忆。");
    assert_eq!(Language::Chinese.nothing_found(), "没找到。");
    assert_eq!(Language::English.no_memories(), "No memories yet.");
    assert_eq!(Language::Chinese.remembered("m4"), "记下了：m4");
    assert_eq!(Language::Chinese.changed("m5"), "改好了：m5");
    assert_eq!(Language::Chinese.cleared(3), "清掉了 3 条。");
    assert_eq!(Language::English.cleared(3), "Cleared 3.");
    assert_eq!(Language::Chinese.dreamed(0, 0, 0, false), "无需整理。");
    assert_eq!(
        Language::Chinese.dreamed(4, 1, 2, false),
        "整理完成：检查 4 条，修改 1 条，作废 2 条。"
    );
    assert_eq!(
        Language::English.dreamed(4, 1, 2, true),
        "Memory organized: 4 checked, 1 revised, 2 retired; summary updated."
    );
    assert_eq!(
        Language::English.dreamed(0, 0, 0, true),
        "Nothing to organize."
    );
}

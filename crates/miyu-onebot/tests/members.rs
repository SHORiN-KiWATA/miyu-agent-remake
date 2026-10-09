//! 群成员的名字缓存（施工 O-22，`onebot.md` 第一条「群消息」第 5 条）：记下的取得到，过了记多久的取不到；按群分开；
//! 再记一次盖掉原来的；`get_group_member_info` 回的群名片空白的取昵称。时刻是交进去的，不等。

use std::time::{Duration, Instant};

use serde_json::json;

use miyu_onebot::onebot::{Members, display_name, member_info};

#[test]
fn a_name_lasts_as_long_as_it_is_kept_and_only_in_its_group() {
    let keep = Duration::from_secs(600);
    let start = Instant::now();
    let mut members = Members::new(keep);
    members.remember(555, 20002, "小林".to_string(), start);
    assert_eq!(members.name(555, 20002, start), Some("小林"));
    assert_eq!(
        members.name(555, 20002, start + keep - Duration::from_millis(1)),
        Some("小林")
    );
    assert_eq!(members.name(555, 20002, start + keep), None, "过了的不用");
    assert_eq!(members.name(666, 20002, start), None, "按群分开");
    assert_eq!(members.name(555, 20003, start), None);
    members.remember(555, 20002, "林".to_string(), start + keep);
    assert_eq!(
        members.name(555, 20002, start + keep),
        Some("林"),
        "再记一次照新的"
    );
}

#[test]
fn the_member_info_asks_napcat_with_its_cache_and_reads_the_card_first() {
    assert_eq!(
        member_info(555, 20002),
        json!({"group_id": 555, "user_id": 20002, "no_cache": false})
    );
    assert_eq!(
        display_name(&json!({"card": "小林", "nickname": "lin"})).as_deref(),
        Some("小林")
    );
    assert_eq!(
        display_name(&json!({"card": "  ", "nickname": "lin"})).as_deref(),
        Some("lin")
    );
    assert_eq!(display_name(&json!({"card": "", "nickname": " "})), None);
    assert_eq!(display_name(&json!(null)), None);
}

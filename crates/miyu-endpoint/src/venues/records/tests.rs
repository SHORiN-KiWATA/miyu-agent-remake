//! 判官看的记录照哪一份字渲染（施工 O-24）：群会话照快照里钉下的，不管核心这时在哪个时区；没有的照这时的。

use super::*;

fn chat(offset: i32) -> GroupChat {
    GroupChat {
        offset,
        no_text: "[no text content]\n".to_string(),
        recent: None,
    }
}

#[test]
fn a_group_reads_with_its_pinned_time_zone() {
    assert_eq!(pick(Some(chat(540)), || Ok(chat(-300))), Ok(chat(540)));
    assert_eq!(pick(None, || Ok(chat(-300))), Ok(chat(-300)));
}

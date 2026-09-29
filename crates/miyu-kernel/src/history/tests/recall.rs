//! 重读的文件的原文（施工 6-5）：交进来的照 blob 找得到；截到第 N 条的那一份（摘要请求照它组装）也带着；换了检查点就清掉。

use super::*;
use crate::id::ContentHash;

#[test]
fn recalled_texts_go_with_the_checkpoint_until_the_next_one() {
    let mut events = two_turns();
    events.extend(compaction_turn(10, 9, 10));
    let mut history = feed(events);
    let blob = ContentHash::of(b"alpha\n");
    history.recall(BTreeMap::from([(blob.clone(), "alpha\n".to_string())]));
    assert_eq!(history.recalled(&blob), Some("alpha\n"));
    assert_eq!(
        history.until(Seq::new(12).unwrap()).recalled(&blob),
        Some("alpha\n")
    );
    let mut ledger_checked = history.clone();
    ledger_checked.append(compacted(13, 12, 10));
    assert_eq!(ledger_checked.recalled(&blob), None, "换了检查点");
}

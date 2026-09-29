//! 侧边栏的「压缩 N 次」「缓存断裂 N 次」（蓝图 `tui.md`「后台命令、子代理和侧边栏」第 7 条）：
//! 只数意外的断裂，压缩、撤销以后的那一次不算，往回看的摘要请求不算。

use super::super::Transcript;
use super::apply;
use crate::core::Push;

fn sent(seen: u64, changed: bool) -> Push {
    Push::Sent {
        seen,
        changed,
        summary: false,
    }
}

#[test]
fn only_an_unexpected_change_of_prefix_counts_as_a_break() {
    let mut t = Transcript::default();
    apply(&mut t, vec![sent(3, false), sent(8, false), sent(12, true)]);
    assert_eq!(t.cache.breaks, 1, "只往后加的不算，前缀变了的算");
    assert_eq!(t.cache.compactions, 0);
}

#[test]
fn the_call_after_a_compaction_or_an_undo_is_excused_once() {
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            sent(3, false),
            Push::Compacted,
            sent(9, true),
            sent(12, true),
        ],
    );
    assert_eq!(t.cache.compactions, 1);
    assert_eq!(t.cache.breaks, 1, "压完那一次不算，再断的算");
    apply(
        &mut t,
        vec![Push::Reverted(vec![10]), sent(15, true), sent(18, false)],
    );
    assert_eq!(t.cache.breaks, 1, "撤销以后那一次不算");
}

#[test]
fn a_summary_looking_back_is_not_a_break_and_keeps_the_excuse_for_the_main_call() {
    let mut t = Transcript::default();
    // 摘要请求只带尾巴前面那一段：看到的比上一次少，比出来少了的那一条算不同。
    apply(
        &mut t,
        vec![
            sent(20, false),
            sent(14, true),
            Push::Compacted,
            sent(22, true),
        ],
    );
    assert_eq!((t.cache.compactions, t.cache.breaks), (1, 0));
}

#[test]
fn a_summary_request_is_known_by_its_flag() {
    // 6-6 上起摘要请求的记录带 `compaction`：看到的不比之前少也认得出，不算断裂，也不用掉压缩给主请求的那一次免数。
    let mut t = Transcript::default();
    apply(
        &mut t,
        vec![
            sent(20, false),
            Push::Sent {
                seen: 20,
                changed: true,
                summary: true,
            },
            Push::Compacted,
            sent(22, true),
        ],
    );
    assert_eq!((t.cache.compactions, t.cache.breaks), (1, 0));
}

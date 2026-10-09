//! 撤销、恢复：撤掉的几轮藏起来、编号不删，另起一条撤销说明；恢复时显示回来，那一条说明藏起（施工 9-8 上）。

use miyu_kernel::testkit::Line;
use miyu_view::Change;

use crate::support::*;

#[test]
fn undo_hides_the_turn_and_redo_shows_it_again() {
    let mut stage = stage();
    stage.model([Line::says("one"), Line::says("two")]);
    stage.say("first\nmore");
    stage.say("second line\nmore");
    let second = stage.turns()[1];
    stage.revert(second);
    let entries = same(&stage);
    let hidden: Vec<String> = entries
        .iter()
        .filter(|e| e.hidden)
        .map(|e| json(e)["kind"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(hidden, ["user", "reply", "end"], "{entries:#?}");
    let notice = json(entries.last().expect("有条目"));
    assert_eq!(notice["what"], "reverted");
    assert_eq!(notice["said"], "second line", "撤掉的第一句的头一行");
    stage.unrevert();
    let entries = same(&stage);
    let hidden: Vec<String> = entries
        .iter()
        .filter(|e| e.hidden)
        .map(|e| json(e)["what"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(hidden, ["reverted"], "恢复了，只藏起那一条说明");
    let (_, changes) = live(&stage);
    assert!(
        changes
            .iter()
            .any(|c| matches!(c, Change::Hidden { hidden: false, .. }))
    );
}

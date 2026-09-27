//! 判法表的每一格（`11-权限与沙盒.md` 第二节，施工 4-3 下）：纯的判法，不碰磁盘。

use super::*;

#[test]
fn every_cell_of_the_table() {
    let (full, work, read_only) = (Effective::Full, Effective::Workspace, Effective::ReadOnly);
    let cases = [
        // 数据根：哪一级都拒绝。
        (Zone::Forbidden, full, false, Mark::Forbidden),
        (Zone::Forbidden, full, true, Mark::Forbidden),
        (Zone::Forbidden, work, false, Mark::Forbidden),
        (Zone::Forbidden, read_only, false, Mark::Forbidden),
        // 能读能写的那几片。
        (Zone::Writable, full, true, Mark::Allow),
        (Zone::Writable, work, false, Mark::Allow),
        (Zone::Writable, work, true, Mark::Allow),
        (Zone::Writable, read_only, false, Mark::Allow),
        (Zone::Writable, read_only, true, Mark::ReadOnly),
        // 只能读的那几片。
        (Zone::Readable, full, true, Mark::Allow),
        (Zone::Readable, work, false, Mark::Allow),
        (Zone::Readable, work, true, Mark::Ask),
        (Zone::Readable, read_only, false, Mark::Allow),
        (Zone::Readable, read_only, true, Mark::ReadOnly),
        // 边界以外。
        (Zone::Outside, full, false, Mark::Allow),
        (Zone::Outside, full, true, Mark::Allow),
        (Zone::Outside, work, false, Mark::Ask),
        (Zone::Outside, work, true, Mark::Ask),
        (Zone::Outside, read_only, false, Mark::Ask),
        (Zone::Outside, read_only, true, Mark::ReadOnly),
    ];
    for (zone, level, write, expected) in cases {
        assert_eq!(
            mark(level, zone, write),
            expected,
            "{zone:?} {level:?} write={write}"
        );
    }
}

#[test]
fn the_level_in_effect() {
    let permission = |level: Level, read_only: bool| Permission { level, read_only };
    assert_eq!(effective(&permission(Level::Full, false)), Effective::Full);
    assert_eq!(
        effective(&permission(Level::Workspace, false)),
        Effective::Workspace
    );
    assert_eq!(
        effective(&permission(Level::Full, true)),
        Effective::ReadOnly
    );
    // 不认识的级别按最严的算。
    assert_eq!(
        effective(&permission(Level::Other("root".to_string()), false)),
        Effective::ReadOnly
    );
}

#[test]
fn a_call_without_paths_goes_by_what_it_does() {
    // 执行命令：M5 之前工作区这一级放行，只读时问人，问的时候不提规则。
    assert_eq!(
        untargeted(Effective::Workspace, "shell", Access::Execute),
        Verdict::Allow
    );
    assert_eq!(
        untargeted(Effective::Full, "shell", Access::Execute),
        Verdict::Allow
    );
    let Verdict::Ask {
        module: asker,
        access,
        rule,
        detail,
    } = untargeted(Effective::ReadOnly, "shell", Access::Execute)
    else {
        panic!("只读时执行命令要问人");
    };
    assert_eq!(asker, module());
    assert_eq!(access, Access::Execute);
    assert_eq!(rule, None);
    assert_eq!(
        detail.map(|detail| detail.get().to_string()).as_deref(),
        Some(r#"{"tool":"shell"}"#)
    );
    // 读写不报路径的放行；联网这些 M4 还没有的，除了完全放开都问人。
    assert_eq!(
        untargeted(Effective::ReadOnly, "x", Access::Read),
        Verdict::Allow
    );
    assert_eq!(
        untargeted(Effective::Workspace, "x", Access::Write),
        Verdict::Allow
    );
    assert!(matches!(
        untargeted(Effective::Workspace, "fetch", Access::Network),
        Verdict::Ask { .. }
    ));
    assert_eq!(
        untargeted(Effective::Full, "fetch", Access::Network),
        Verdict::Allow
    );
}

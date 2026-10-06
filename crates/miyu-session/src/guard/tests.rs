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
        // 边界以外：读哪一级都放行（施工 5-4 上）。
        (Zone::Outside, full, false, Mark::Allow),
        (Zone::Outside, full, true, Mark::Allow),
        (Zone::Outside, work, false, Mark::Allow),
        (Zone::Outside, work, true, Mark::Ask),
        (Zone::Outside, read_only, false, Mark::Allow),
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
fn a_command_goes_by_whether_the_sandbox_can_be_used() {
    let (full, work, read_only) = (Effective::Full, Effective::Workspace, Effective::ReadOnly);
    // 沙盒能用：哪一级都放行，工作区、只读在沙盒里跑（施工 5-4 上）。
    for level in [full, work, read_only] {
        assert_eq!(
            untargeted(level, "shell", Access::Execute, true, &[]),
            Verdict::Allow,
            "{level:?}"
        );
    }
    // 用不了：完全放开照样放行，别的两级问人，问的时候不提规则。
    assert_eq!(
        untargeted(full, "shell", Access::Execute, false, &[]),
        Verdict::Allow
    );
    for level in [work, read_only] {
        let Verdict::Ask {
            module: asker,
            access,
            rule,
            detail,
        } = untargeted(level, "shell", Access::Execute, false, &[])
        else {
            panic!("沙盒用不了，{level:?} 执行命令要问人");
        };
        assert_eq!(asker, module());
        assert_eq!(access, Access::Execute);
        assert_eq!(rule, None);
        assert_eq!(
            detail.map(|detail| detail.get().to_string()).as_deref(),
            Some(r#"{"sandbox":false,"tool":"shell"}"#)
        );
    }
}

#[test]
fn a_call_without_paths_goes_by_what_it_does() {
    // 读写不报路径的放行；联网这些还没有的，除了完全放开都问人，沙盒能用也问。
    for sandboxed in [false, true] {
        assert_eq!(
            untargeted(Effective::ReadOnly, "x", Access::Read, sandboxed, &[]),
            Verdict::Allow
        );
        assert_eq!(
            untargeted(Effective::Workspace, "x", Access::Write, sandboxed, &[]),
            Verdict::Allow
        );
        assert!(matches!(
            untargeted(
                Effective::Workspace,
                "fetch",
                Access::Network,
                sandboxed,
                &[]
            ),
            Verdict::Ask { .. }
        ));
        assert_eq!(
            untargeted(Effective::Full, "fetch", Access::Network, sandboxed, &[]),
            Verdict::Allow
        );
    }
}

/// 问人时的说明（施工 D-4）：工具交的几格并进来，键照字母排；工具名由权限策略写，工具交的同名格盖不过它；执行类的写明
/// 这一次不在沙盒里跑，别的不写。
#[test]
fn the_detail_takes_what_the_tool_says() {
    let asking = [
        ("title", "Create a file".to_string()),
        ("command", "touch ~/x".to_string()),
        ("tool", "not me".to_string()),
    ];
    assert_eq!(
        described("shell", &Access::Execute, &asking).get(),
        r#"{"command":"touch ~/x","sandbox":false,"title":"Create a file","tool":"shell"}"#
    );
    assert_eq!(
        described("fetch", &Access::Network, &[]).get(),
        r#"{"tool":"fetch"}"#
    );
    // 沙盒用不了时每条命令都问：说明照样带标题和命令（D-1 补）。
    let Verdict::Ask { detail, .. } = untargeted(
        Effective::Workspace,
        "shell",
        Access::Execute,
        false,
        &asking[..2],
    ) else {
        panic!("沙盒用不了，要问人");
    };
    assert_eq!(
        detail.map(|detail| detail.get().to_string()).as_deref(),
        Some(r#"{"command":"touch ~/x","sandbox":false,"title":"Create a file","tool":"shell"}"#)
    );
}

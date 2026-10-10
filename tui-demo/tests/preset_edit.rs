//! 配置页改预设（蓝图 `tui.md`「配置页」第 38、39 条，核心 P-3）：`a` 填名字就建、功能照旧版空格切、`Ctrl+A` 全开全关、
//! `d` 删除；编号核心起，界面上不出现。

mod support;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use miyu_session::testkit::{Play, Script};
use support::{Home, Tui};

/// 新建的第一个预设的文件（编号核心起：`preset-1`）。
fn file(home: &Home) -> PathBuf {
    home.root().join("home/alice/presets/preset-1.toml")
}

/// 文件里的字；没有的是空的。
fn text(home: &Home) -> String {
    std::fs::read_to_string(file(home)).unwrap_or_default()
}

/// 等 `check` 成立，最多 [`support::WAIT`]；等不到的把屏幕打出来。
fn until(tui: &mut Tui, what: &str, check: impl Fn() -> bool) {
    let end = Instant::now() + support::WAIT;
    while !check() {
        assert!(
            Instant::now() < end,
            "等不到{what}，屏幕是：\n{}",
            tui.lines().join("\n")
        );
        tui.pump(Duration::from_millis(100));
    }
}

/// 起在配置页，挪到「预设」进去（主菜单第四项：通用、供应商和模型、人格、预设；「权限」2026-10-10 并进了「通用」），`a`
/// 建一个「我的预设」。
fn made(home: &Home) -> Tui {
    let mut tui = home.tui_args("zh_CN.UTF-8", &["config"]);
    tui.wait_for("决定开关哪些功能");
    for _ in 0..3 {
        tui.key(b"j");
    }
    tui.key(b"\r");
    tui.wait_for("全部功能");
    tui.key(b"a");
    tui.wait_for("新预设名称");
    tui.type_text("我的预设");
    tui.key(b"\r");
    until(&mut tui, "建好的文件", || {
        text(home).contains("我的预设")
    });
    // 建好开它的窗：名字、功能；编号不出现。
    tui.wait_for("功能");
    assert!(!tui.shows("preset-1"), "{}", tui.lines().join("\n"));
    tui
}

#[test]
fn a_new_preset_has_its_features_toggled_with_space_and_is_deleted() {
    let home = Home::new(Script::new([Play::Says("好。")]));
    let mut tui = made(&home);
    // 新建的功能先全开：一项都没写。光标从名字起，下一行是第一项功能，空格切掉。
    assert!(!text(&home).contains("[software]"), "{}", text(&home));
    tui.key(b"j");
    tui.key(b" ");
    until(&mut tui, "切掉的那一项", || {
        text(&home).contains("= false")
    });
    tui.wait_for("[ ]");
    // 删除：问一句，确定了文件没了，写「已删除」。
    tui.key(b"d");
    tui.wait_for("删除预设");
    tui.key(b"h");
    tui.key(b"\r");
    until(&mut tui, "删掉的文件", || !file(&home).exists());
    tui.wait_for("已删除");
}

#[test]
fn ctrl_a_turns_every_feature_off_then_on_again() {
    let home = Home::new(Script::new([Play::Says("好。")]));
    let mut tui = made(&home);
    // 都开着：全关。
    tui.key(b"\x01");
    until(&mut tui, "全关", || {
        let t = text(&home);
        t.contains("= false") && !t.contains("= true")
    });
    // 有没开的：全开。
    tui.key(b"\x01");
    until(&mut tui, "全开", || !text(&home).contains("= false"));
}

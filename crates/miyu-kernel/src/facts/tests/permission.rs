//! 权限那一块（施工 2-7 补，`docs/blueprint/kernel/request.md`「事实」第 2 条）：切换那一份模板造的时候就查；它写出两级；
//! 边界上比的是级别不是原文：没有上一块的写平常那一份，级别一样的不写，不一样的用切换那一份、带上一块的级别；上一块本身是
//! 切换那一份的，照它新的那一级算；以前的快照没有这份模板的，和以前逐字节比一样；认不出的上一块、模块注入的、压缩和撤销
//! 拿走的，都当没有上一块。

use super::*;

/// 替身的模板加上切换那一份：短，一眼认得出是哪个字段。
fn switching() -> FactTemplates {
    FactTemplates::new(
        r#"<e t="{time}" z="{timezone}" d="{cwd}"/>"#,
        r#"<p l="{level}"/>"#,
        "<cut/>",
        None,
        Some(r#"<p l="{level}" was="{previous}"/>"#),
    )
    .unwrap()
}

/// 以前造的快照：没有切换那一份。
fn older() -> FactTemplates {
    FactTemplates::new(
        r#"<e t="{time}" z="{timezone}" d="{cwd}"/>"#,
        r#"<p l="{level}"/>"#,
        "<cut/>",
        None,
        None,
    )
    .unwrap()
}

/// 平常那一份写出的 `level`。
fn plain(level: &str) -> String {
    format!(r#"<p l="{level}"/>"#)
}

/// 切换那一份写出的从 `previous` 切到 `level`。
fn switched(level: &str, previous: &str) -> String {
    format!(r#"<p l="{level}" was="{previous}"/>"#)
}

/// 工作区、完全放开、只读开着（常用的那一级是完全放开）。
fn workspace() -> Permission {
    permission(Level::Workspace, false)
}

fn full() -> Permission {
    permission(Level::Full, false)
}

fn read_only() -> Permission {
    permission(Level::Full, true)
}

/// 这个边界上权限那一块注入什么；不注入的是 `None`。环境那一块不看。
fn told(templates: &FactTemplates, log: &Log, permission: &Permission) -> Option<String> {
    templates
        .boundary(
            &log.history,
            now(),
            &environment("~/src/miyu"),
            permission,
            &session(),
        )
        .into_iter()
        .find(|fact| fact.kind.as_str() == "permission")
        .map(|fact| fact.text)
}

/// 开一个回合，内核在里面记下权限那一块 `text`。
fn saw(log: &mut Log, text: &str) {
    log.start();
    log.inject(KERNEL, "permission", text);
    log.end();
}

#[test]
fn the_changed_template_is_checked_when_made() {
    let asks = |changed: &str| {
        FactTemplates::new(
            r#"<e t="{time}"/>"#,
            r#"<p l="{level}"/>"#,
            "<cut/>",
            None,
            Some(changed),
        )
    };
    let wrong = asks(r#"<p l="{level}" t="{time}"/>"#).unwrap_err();
    assert!(wrong.why.contains("time"), "{wrong}");
    assert!(asks(r#"<p l="{level"/>"#).is_err());
    // 只用其中几个字段也行。
    assert!(asks(r#"<p l="{level}"/>"#).is_ok());
}

#[test]
fn the_changed_block_names_the_level_in_effect_and_the_one_before() {
    let block = switching()
        .permission_changed(&read_only(), "workspace")
        .unwrap();
    assert_eq!(block.kind.as_str(), "permission");
    assert_eq!(block.text, switched("read_only", "workspace"));
    assert_eq!(older().permission_changed(&full(), "workspace"), None);
}

#[test]
fn with_no_block_before_it_the_plain_one_is_told() {
    let mut log = Log::new();
    log.start();
    assert_eq!(told(&switching(), &log, &full()), Some(plain("full")));
}

#[test]
fn the_same_level_is_not_told_again() {
    let mut log = Log::new();
    saw(&mut log, &plain("workspace"));
    log.start();
    assert_eq!(told(&switching(), &log, &workspace()), None);
}

#[test]
fn another_level_is_told_with_the_one_before() {
    let mut log = Log::new();
    saw(&mut log, &plain("workspace"));
    log.start();
    assert_eq!(
        told(&switching(), &log, &full()),
        Some(switched("full", "workspace"))
    );
    assert_eq!(
        told(&switching(), &log, &read_only()),
        Some(switched("read_only", "workspace")),
        "只读开着，实际生效的是只读"
    );
}

#[test]
fn a_changed_block_counts_as_its_new_level() {
    let mut log = Log::new();
    saw(&mut log, &plain("workspace"));
    saw(&mut log, &switched("full", "workspace"));
    log.start();
    assert_eq!(told(&switching(), &log, &full()), None);
    assert_eq!(
        told(&switching(), &log, &read_only()),
        Some(switched("read_only", "full"))
    );
    assert_eq!(
        told(&switching(), &log, &workspace()),
        Some(switched("workspace", "full")),
        "切回去也照最近那一块比"
    );
}

#[test]
fn older_templates_tell_the_plain_block_as_before() {
    let mut log = Log::new();
    saw(&mut log, &plain("workspace"));
    log.start();
    assert_eq!(told(&older(), &log, &full()), Some(plain("full")));
    assert_eq!(told(&older(), &log, &workspace()), None);
}

#[test]
fn a_block_it_cannot_read_is_compared_by_its_text() {
    let mut log = Log::new();
    saw(&mut log, "<p>odd</p>");
    log.start();
    assert_eq!(
        told(&switching(), &log, &workspace()),
        Some(plain("workspace"))
    );
}

#[test]
fn blocks_from_modules_do_not_count() {
    let mut log = Log::new();
    log.start();
    log.inject(MEMORY, "permission", &plain("workspace"));
    assert_eq!(told(&switching(), &log, &full()), Some(plain("full")));
}

#[test]
fn after_a_compaction_the_plain_block_is_told() {
    let mut log = Log::new();
    saw(&mut log, &plain("workspace"));
    log.start();
    log.compact();
    assert_eq!(told(&switching(), &log, &full()), Some(plain("full")));
}

#[test]
fn after_undoing_the_turns_that_told_it_the_block_before_counts() {
    let mut log = Log::new();
    saw(&mut log, &plain("workspace"));
    let second = log.start();
    log.inject(KERNEL, "permission", &switched("full", "workspace"));
    log.end();
    log.revert(second);
    log.start();
    assert_eq!(
        told(&switching(), &log, &read_only()),
        Some(switched("read_only", "workspace"))
    );
    // 连第一块也撤掉了：没有上一块。
    let mut log = Log::new();
    let first = log.start();
    log.inject(KERNEL, "permission", &plain("workspace"));
    log.end();
    log.revert(first);
    log.start();
    assert_eq!(told(&switching(), &log, &full()), Some(plain("full")));
}

#[test]
fn every_level_can_be_the_one_before() {
    for (before, now, previous) in [
        (read_only(), workspace(), "read_only"),
        (workspace(), read_only(), "workspace"),
        (full(), workspace(), "full"),
    ] {
        for seen in [
            switching().permission(&before).text,
            switching()
                .permission_changed(&before, "workspace")
                .unwrap()
                .text,
        ] {
            let mut log = Log::new();
            saw(&mut log, &seen);
            log.start();
            let level = effective_level(&now);
            assert_eq!(
                told(&switching(), &log, &now),
                Some(switched(level, previous)),
                "{seen}"
            );
        }
    }
}

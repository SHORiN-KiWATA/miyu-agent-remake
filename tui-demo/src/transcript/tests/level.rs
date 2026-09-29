//! 权限级别（蓝图 `tui.md`「权限级别」）：Tab 轮换算下一档，界面照核心推来的 `session.policy_changed` 画；你说的话
//! 记着发出去时的级别。

use super::{apply, policy};
use crate::config::Config;
use crate::transcript::Transcript;

#[test]
fn shift_tab_cycles_workspace_full_read_only() {
    let mut t = Transcript::default();
    let order = Config::builtin().unwrap().layout.level_cycle;
    let mut seen = Vec::new();
    // 只算下一档：切到哪一档由核心推来的 session.policy_changed 定，这里照它推（「权限级别」第 2 条）。
    for _ in 0..4 {
        let next = t.next_level(&order);
        seen.push(next);
        apply(&mut t, vec![policy(next)]);
    }
    use crate::core::Level;
    assert_eq!(
        seen,
        vec![Level::Full, Level::ReadOnly, Level::Workspace, Level::Full]
    );
}

#[test]
fn what_you_said_keeps_the_level_it_was_sent_with() {
    let mut t = Transcript::default();
    t.user("你好".into(), Vec::new());
    let order = Config::builtin().unwrap().layout.level_cycle;
    let next = t.next_level(&order);
    apply(&mut t, vec![policy(next)]);
    t.user("再来".into(), Vec::new());
    let levels: Vec<_> = t.entries.iter().map(|e| e.level).collect();
    use crate::core::Level;
    assert_eq!(levels, vec![Some(Level::Workspace), Some(Level::Full)]);
}

//! 带角色扮演提示的策略（施工 P-1 补，`docs/blueprint/kernel/request.md`「事实」）：探针 `probe_reminder.rs` 和随机日志里
//! 三分之一的种子用。

use miyu_kernel::session::Policy;

/// 人格写的提示：探针里的一句。
pub const SAID: &str = "Stay soft and brief.";

/// 风格锁的原文。
pub const STYLE_LOCK: &str = include_str!("../../../../resources/core/style-lock.txt");

/// 拼好的那一块：出厂的包装、提示、换行（`policy.md`「拼」第 4 条）。
pub fn block() -> String {
    format!(
        "{}{SAID}\n{}",
        include_str!("../../../../resources/core/facts/reminder-open.txt"),
        include_str!("../../../../resources/core/facts/reminder-close.txt")
    )
}

/// 策略：system 最后空一行接风格锁，事实模板带上那一块；别的和 [`super::policy`] 一样。
pub fn policy() -> Policy {
    let system = format!(
        "{}\n\n{}\n\n{}",
        super::PERSONA,
        super::LINES.trim_end(),
        STYLE_LOCK.trim_end()
    );
    let mut policy = super::policy_with(system);
    policy.facts = policy.facts.with_reminder(Some(block()));
    policy
}

//! 执行器替工具写的字（施工 P-1 三补）：执行工具的端口替工具写的两句（[`RunTexts`]）、权限策略的几句（[`GuardTexts`]），取自
//! 会话的策略快照。执行工具的端口和权限策略拿着同一份；换快照时一起换（`actor/persona.rs`），和内核换策略在同一个回合开头。

use std::sync::{PoisonError, RwLock, RwLockReadGuard};

use miyu_policy::{GuardTexts, RunTexts};

/// 执行器替工具写的字。
pub(crate) struct Lettering {
    run: RwLock<RunTexts>,
    guard: RwLock<GuardTexts>,
}

impl Lettering {
    /// 照快照取好的两份。
    pub(crate) fn new(run: RunTexts, guard: GuardTexts) -> Lettering {
        Lettering {
            run: RwLock::new(run),
            guard: RwLock::new(guard),
        }
    }

    /// 执行工具的端口替工具写的两句。
    pub(crate) fn run(&self) -> RwLockReadGuard<'_, RunTexts> {
        self.run.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// 权限策略的几句。
    pub(crate) fn guard(&self) -> RwLockReadGuard<'_, GuardTexts> {
        self.guard.read().unwrap_or_else(PoisonError::into_inner)
    }

    /// 换快照了，换成新快照的两份。
    pub(crate) fn set(&self, run: RunTexts, guard: GuardTexts) {
        *self.run.write().unwrap_or_else(PoisonError::into_inner) = run;
        *self.guard.write().unwrap_or_else(PoisonError::into_inner) = guard;
    }
}

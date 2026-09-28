//! `miyu sandbox setup`、`remove` 在 Windows 上怎么走（`docs/blueprint/sandbox/windows.md`「怎么走」第 2、3、6 条）：
//! 替谁装、是不是管理员；是的直接干，不是的起一个提升过的自己、等它、读它留下的原因。
//!
//! 只经 [`Machine`] 这个接口碰真的机器：Windows 上是真的那一套（`sandbox.rs`），测试里换替身，三个平台都跑得到。

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

use miyu_sandbox::install::{Elevation, InstallError, Owner};

use super::{Said, Which};

/// 装沙盒要碰的那台机器。
pub(crate) trait Machine {
    /// 当前是不是管理员。
    fn elevated(&self) -> io::Result<bool>;
    /// 自己是谁：本人的数据根（建好骨架）和 SID。
    fn me(&self) -> Result<Owner, InstallError>;
    /// 起一个提升过的自己，参数是 `args`，等它退出：交回它的退出码。
    fn elevate(&self, args: &[OsString]) -> Result<u32, Elevation>;
    /// 干活：装或卸，替 `owner`。
    fn apply(&self, which: Which, owner: &Owner) -> Result<(), InstallError>;
    /// 提升过的自己没成：把原因写给等它的那一个。
    fn report(&self, owner: &Owner, error: &InstallError) -> Result<(), InstallError>;
    /// 等它的那一个读回原因。
    fn reported(&self, owner: &Owner) -> Result<Option<InstallError>, InstallError>;
    /// 删掉留下的原因。
    fn clear(&self, owner: &Owner) -> Result<(), InstallError>;
}

/// 走一遍：`given` 是只给提升过的自己用的那两个参数（数据根、SID），带了的就是替参数里的人装。交回要说的那一句。
pub(crate) fn run(machine: &impl Machine, which: Which, given: Option<(PathBuf, String)>) -> Said {
    let elevated_self = given.is_some();
    let owner = match given {
        Some((home, sid)) => Owner::new(home, sid),
        None => machine.me(),
    };
    let owner = match owner {
        Ok(owner) => owner,
        Err(error) => return Said::Failed(which, error),
    };
    match machine.elevated() {
        Err(error) => Said::Failed(which, InstallError::failed("elevate", error)),
        Ok(true) => match machine.apply(which, &owner) {
            Ok(()) => Said::Done(which),
            Err(error) => {
                // 等它的那一个看不到这里说的话：原因写给它。写不成的就算了，照样说。
                if elevated_self {
                    #[expect(
                        clippy::let_underscore_must_use,
                        reason = "写不成原因也没有别处可说：等它的那一个会说出退出码"
                    )]
                    let _ = machine.report(&owner, &error);
                }
                Said::Failed(which, error)
            }
        },
        Ok(false) => elevate(machine, which, &owner),
    }
}

/// 不是管理员：清掉上次留下的原因，起一个提升过的自己，照它的结局说。
fn elevate(machine: &impl Machine, which: Which, owner: &Owner) -> Said {
    if let Err(error) = machine.clear(owner) {
        return Said::Failed(which, error);
    }
    match machine.elevate(&child_args(which, owner)) {
        Ok(0) => Said::Done(which),
        Ok(code) => {
            let error = match machine.reported(owner) {
                Ok(Some(error)) => error,
                Ok(None) => InstallError::failed("elevate", format!("exit code {code}")),
                Err(error) => error,
            };
            #[expect(
                clippy::let_underscore_must_use,
                reason = "原因已经读到了；清不掉的下次起之前再清"
            )]
            let _ = machine.clear(owner);
            Said::Failed(which, error)
        }
        Err(Elevation::Cancelled) => Said::NeedsAdmin(which),
        Err(Elevation::Failed(error)) => {
            Said::Failed(which, InstallError::failed("elevate", error))
        }
    }
}

/// 起提升过的自己时给的参数：`sandbox <setup 或 remove> --owner-home <数据根> --owner-sid <SID>`。
pub(crate) fn child_args(which: Which, owner: &Owner) -> Vec<OsString> {
    vec![
        "sandbox".into(),
        which.name().into(),
        "--owner-home".into(),
        owner.home().into(),
        "--owner-sid".into(),
        owner.sid().into(),
    ]
}

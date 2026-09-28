//! `miyu sandbox setup`、`miyu sandbox remove` 给人看的字（`docs/blueprint/sandbox/windows.md`「给人看的字」，施工 5-8）。
//! 系统的原话、路径里的控制字符换成 `�`：它们可能混着终端的控制序列。

use miyu_sandbox::install::{InstallError, USER};
use miyu_store::human::clean;

use super::Language;
use crate::sandbox::{Said, Which};

impl Language {
    /// 走完了要说的那一句。
    pub(crate) fn sandbox(&self, said: &Said) -> String {
        let chinese = *self == Language::Chinese;
        match said {
            Said::Done(Which::Setup) => match chinese {
                true => "沙盒用户建好了。".to_string(),
                false => "The sandbox user is set up.".to_string(),
            },
            Said::Done(Which::Remove) => match chinese {
                true => "沙盒用户撤掉了。".to_string(),
                false => "The sandbox user is removed.".to_string(),
            },
            Said::NotNeeded => match chinese {
                true => "这个平台不用装沙盒。".to_string(),
                false => "Nothing to set up on this platform.".to_string(),
            },
            Said::NeedsAdmin(which) => match chinese {
                true => format!(
                    "要管理员权限：用管理员身份跑 miyu sandbox {}。",
                    which.name()
                ),
                false => format!(
                    "Administrator rights are needed: run miyu sandbox {} as administrator.",
                    which.name()
                ),
            },
            Said::Failed(which, error) => self.sandbox_failed(*which, error),
        }
    }

    /// 没成的那一句。
    fn sandbox_failed(&self, which: Which, error: &InstallError) -> String {
        let chinese = *self == Language::Chinese;
        match error {
            InstallError::Administrator => match chinese {
                true => format!("已经有一个叫 {USER} 的管理员账号，不动它。"),
                false => {
                    format!(
                        "An administrator account named {USER} already exists; leaving it alone."
                    )
                }
            },
            InstallError::NotDataRoot { path } => match chinese {
                true => format!("{} 不是 Miyu 的数据根。", clean(path)),
                false => format!("{} is not a Miyu data root.", clean(path)),
            },
            InstallError::Failed { step, detail } => {
                let (step, detail) = (clean(step), clean(detail));
                match (chinese, which) {
                    (true, Which::Setup) => format!("装沙盒失败：{step}：{detail}"),
                    (true, Which::Remove) => format!("卸沙盒失败：{step}：{detail}"),
                    (false, Which::Setup) => format!("Sandbox setup failed: {step}: {detail}"),
                    (false, Which::Remove) => format!("Sandbox removal failed: {step}: {detail}"),
                }
            }
        }
    }
}

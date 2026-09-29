//! `miyu sandbox setup`、`miyu sandbox remove` 给人看的字（`docs/blueprint/sandbox/windows.md`「给人看的字」，施工 5-8）。
//! 系统的原话、路径里的控制字符换成 `�`：它们可能混着终端的控制序列。

use miyu_sandbox::install::{InstallError, USER};
use miyu_sandbox::{Platform, Unusable};
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

    /// `miyu ask` 开头，沙盒用不了的那一句（施工 5-4 下）：原因 `reason` 是协议上的写法，照这台机器的系统 `platform`
    /// 写成人话；不认得的原因照原样写进括号里。
    pub(crate) fn unsandboxed(&self, reason: &str, platform: Platform) -> String {
        let why = match Unusable::from_code(reason) {
            Some(known) => self.why_unsandboxed(known, platform).to_string(),
            None => clean(reason),
        };
        match self {
            Language::Chinese => {
                format!("· 沙盒用不了（{why}）：执行命令要你确认，miyu ask 里确认不了")
            }
            Language::English => format!(
                "· Sandbox unavailable ({why}): commands need your approval, which cannot be given in miyu ask"
            ),
        }
    }

    /// 沙盒用不了的原因，写成人话：原因和怎么修。
    fn why_unsandboxed(&self, reason: Unusable, platform: Platform) -> &'static str {
        let chinese = *self == Language::Chinese;
        match (reason, platform) {
            (Unusable::HelperMissing, _) => match chinese {
                true => "主程序旁边没有 miyu-sandbox：重装一次 Miyu",
                false => "miyu-sandbox is missing beside the main program: reinstall Miyu",
            },
            (Unusable::HelperFailed, _) => match chinese {
                true => "miyu-sandbox 跑不起来：重装一次 Miyu",
                false => "miyu-sandbox does not run: reinstall Miyu",
            },
            (Unusable::NoMechanism, Platform::Linux) => match chinese {
                true => "内核没有能用的 Landlock：要 Linux 5.13 起，启动参数的 lsm= 里开着",
                false => {
                    "the kernel has no usable Landlock: Linux 5.13 or later, enabled in the lsm= boot parameter"
                }
            },
            (Unusable::NoMechanism, Platform::Macos) => match chinese {
                true => "装不上 Seatbelt 配置，Miyu 可能跑在别的沙盒里",
                false => {
                    "the Seatbelt profile cannot be applied; Miyu may be running inside another sandbox"
                }
            },
            (Unusable::NoMechanism, Platform::Windows) => match chinese {
                true => "这一版在 Windows 上还不能把命令关进沙盒",
                false => "this version cannot sandbox commands on Windows yet",
            },
            (Unusable::NoMechanism, Platform::Other) => match chinese {
                true => "这个系统上没有能用的沙盒",
                false => "no sandbox is available on this system",
            },
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

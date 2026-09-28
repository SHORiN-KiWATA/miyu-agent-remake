//! Linux 上的收紧（`docs/blueprint/sandbox.md`，蓝图各平台一页）：施工 5-2 起文件（Landlock），5-3 挂载命名空间，
//! 5-6 网络命名空间和 seccomp。施工 5-1 还什么都不收紧。

use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

use miyu_sandbox::Spec;

/// 这台机器上能用上的收紧手段：`probe` 印的。
pub(crate) fn mechanisms() -> Vec<String> {
    Vec::new()
}

/// 照规格 `spec` 收紧，再换成那条命令。
pub(crate) fn run(_spec: &Spec, program: &OsStr, args: &[OsString]) -> ExitCode {
    crate::unix::exec(program, args)
}

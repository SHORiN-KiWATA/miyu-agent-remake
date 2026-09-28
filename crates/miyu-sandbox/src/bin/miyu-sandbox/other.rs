//! 别的 Unix（FreeBSD 这些）：没有收紧的手段，不在发行的平台里。照样换成那条命令，探测时手段是空的，核心照
//! 「沙盒用不了」办（施工 5-4）。

use std::ffi::{OsStr, OsString};
use std::process::ExitCode;

use miyu_sandbox::Spec;

/// 这台机器上能用上的收紧手段：没有。
pub(crate) fn mechanisms() -> Vec<String> {
    Vec::new()
}

/// 不收紧，换成那条命令。
pub(crate) fn run(_spec: &Spec, program: &OsStr, args: &[OsString]) -> ExitCode {
    crate::unix::exec(program, args)
}

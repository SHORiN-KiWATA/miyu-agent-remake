//! Windows 上的收紧（`docs/blueprint/sandbox.md`，蓝图各平台一页）：施工 5-8 装沙盒用户，5-9 受限令牌、访问控制、
//! 单独的桌面。Windows 换不了进程：起那条命令、等它，照它的退出码退出。施工 5-1 还什么都不收紧。

use std::ffi::{OsStr, OsString};
use std::process::{Command, ExitCode};

use miyu_sandbox::{EXIT_HELPER, Spec};

/// 这台机器上能用上的收紧手段：`probe` 印的。
pub(crate) fn mechanisms() -> Vec<String> {
    Vec::new()
}

/// 照规格 `spec` 收紧，起那条命令、等它：退出码是 32 位的，照原样交回去。
pub(crate) fn run(_spec: &Spec, program: &OsStr, args: &[OsString]) -> ExitCode {
    match Command::new(program).args(args).status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(i32::from(EXIT_HELPER))),
        Err(error) => crate::cannot_run(program, &error),
    }
}

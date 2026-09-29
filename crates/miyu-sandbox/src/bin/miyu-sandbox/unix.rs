//! Unix 上换成那条命令（`exec`）：同一个进程，工作目录、环境变量、标准输入输出、进程组都照旧。Linux、macOS 收紧
//! 以后都走这里。

use std::ffi::{OsStr, OsString};
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};

/// 换成 `program`：换成了就不回来，换不成才交回退出码。
pub(crate) fn exec(program: &OsStr, args: &[OsString]) -> ExitCode {
    let error = Command::new(program).args(args).exec();
    crate::cannot_run(program, &error)
}

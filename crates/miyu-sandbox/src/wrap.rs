//! 照规格把一条命令包成交给助手的样子（`docs/blueprint/sandbox.md`「怎么走」第 3 条）：
//! `<助手> run --spec <规格的 JSON> -- <程序> <参数…>`。工作目录、环境变量、标准输入输出由起命令的一方照旧设。

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::Spec;

/// 一次调用带的沙盒：助手在哪，规格是什么（`docs/blueprint/tools/interface.md`「一次调用交给工具的」）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sandboxed {
    /// 助手 `miyu-sandbox` 的路径。
    pub helper: PathBuf,
    /// 规格。
    pub spec: Spec,
}

/// 要执行的程序 `program` 和参数 `args`，包成交给助手的：交回助手的路径和给它的参数。
///
/// # Errors
///
/// 规格写不成 JSON：路径不是 UTF-8 的。
pub fn argv(
    sandboxed: &Sandboxed,
    program: &Path,
    args: &[OsString],
) -> Result<(PathBuf, Vec<OsString>), serde_json::Error> {
    let mut out: Vec<OsString> = vec![
        "run".into(),
        "--spec".into(),
        sandboxed.spec.to_json()?.into(),
        "--".into(),
        program.as_os_str().to_os_string(),
    ];
    out.extend(args.iter().cloned());
    Ok((sandboxed.helper.clone(), out))
}

#[cfg(test)]
mod tests;

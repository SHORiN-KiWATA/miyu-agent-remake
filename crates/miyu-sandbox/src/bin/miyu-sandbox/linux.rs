//! Linux 上的收紧（`docs/blueprint/sandbox/linux.md`）：施工 5-2 起文件用 Landlock。挂载命名空间（5-3）、网络命名
//! 空间和 seccomp（5-6）以后加。
//!
//! Landlock 只能放行，不能在放行的范围里再挖掉一块：规格里只读的、藏起来的落在放行的范围里，这一步收不住，拒绝
//! 执行，不假装收住了。

mod landlock;
#[cfg(test)]
mod tests;

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use miyu_sandbox::Spec;

/// 这台机器上能用上的收紧手段：`probe` 印的。
pub(crate) fn mechanisms() -> Vec<String> {
    match landlock::available() {
        Ok(()) => vec!["landlock".to_string()],
        Err(_) => Vec::new(),
    }
}

/// 照规格 `spec` 收紧，再换成那条命令。收不住的说原话，退出 125，不跑命令。
pub(crate) fn run(spec: &Spec, program: &OsStr, args: &[OsString]) -> ExitCode {
    if let Err(why) = check(spec).and_then(|()| landlock::confine(spec)) {
        return crate::fail(&format!("cannot confine: {why}"));
    }
    crate::unix::exec(program, args)
}

/// 查规格收不收得住：只读的落在能写的里面、藏起来的落在放行的里面，Landlock 挖不掉（5-3 起挂载命名空间补上）。
fn check(spec: &Spec) -> Result<(), String> {
    if let Some(path) = spec.readonly.iter().find(|path| inside(path, &spec.write)) {
        return Err(format!(
            "cannot keep {} read-only inside a writable path",
            path.display()
        ));
    }
    let allowed = |path: &&PathBuf| inside(path, &spec.read) || inside(path, &spec.write);
    if let Some(path) = spec.hidden.iter().find(allowed) {
        return Err(format!(
            "cannot hide {} inside an allowed path",
            path.display()
        ));
    }
    Ok(())
}

/// `path` 是 `allowed` 里某一条本身，或者在它下面。照路径一段段比：`/a/bc` 不在 `/a/b` 下面。
fn inside(path: &Path, allowed: &[PathBuf]) -> bool {
    allowed.iter().any(|base| path.starts_with(base))
}

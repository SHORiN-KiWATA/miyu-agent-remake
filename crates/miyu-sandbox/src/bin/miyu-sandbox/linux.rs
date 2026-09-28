//! Linux 上的收紧（`docs/blueprint/sandbox/linux.md`）：只用 Landlock（施工 5-2 起；5-3 照 DeepSeek 的 dsh 改成整盘能
//! 读、只管写）。不要命名空间、不要 AppArmor（2026-09-29 项目主人定）。
//!
//! Landlock 只能放行，不能在放行的范围里再挖掉一块：「除了藏起来的都能读」是一级级放行做出来的（[`reads`]）；藏起来的
//! 落在能写的里面，挖不掉，拒绝执行，不假装收住了。

mod landlock;
mod reads;
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

/// 查规格收不收得住：藏起来的落在能写的里面，Landlock 挖不掉。能写的落在藏起来的里面没事：它有自己的一条规则。
fn check(spec: &Spec) -> Result<(), String> {
    match spec.hidden.iter().find(|path| inside(path, &spec.write)) {
        Some(path) => Err(format!(
            "cannot hide {} inside a writable path",
            path.display()
        )),
        None => Ok(()),
    }
}

/// `path` 是 `allowed` 里某一条本身，或者在它下面。照路径一段段比：`/a/bc` 不在 `/a/b` 下面。
fn inside(path: &Path, allowed: &[PathBuf]) -> bool {
    allowed.iter().any(|base| path.starts_with(base))
}

//! 规格里的路径换成真实的位置（`docs/blueprint/sandbox/macos.md`「怎么走」第 1 条）。
//!
//! 规格说路径都是真实的位置（`sandbox.md`），助手再换一次，防着给的是经过链接的写法：Seatbelt 照真实的位置比，
//! 写法对不上的规则等于没写，要挡的就漏了。macOS 上 `/tmp`、`/var`、`/etc` 都是 `/private` 下的链接，`TMPDIR` 在
//! `/var/folders/…` 下。

use std::path::{Path, PathBuf};

use miyu_sandbox::Spec;

#[cfg(test)]
mod tests;

/// 换过的规格：配置照它写（`profile.rs`）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Resolved {
    /// 能写的：只用换过的。放开链接本身的话，删了它换一个，下一条命令就放开了别处。
    pub(crate) write: Vec<PathBuf>,
    /// 藏起来的：换过的和原样不一样时两样都写，原样的那一条挡住链接本身，不然删了链接、换一个，就绕过去了。
    pub(crate) hidden: Vec<PathBuf>,
}

/// 查、换规格里的每一条路径。同一样里重复的只写一次。
///
/// # Errors
///
/// 规格里有不是绝对路径的、有 `.`、`..` 这样的段的、有 NUL 的：交回收紧不成时说的原话。
pub(crate) fn resolve(spec: &Spec) -> Result<Resolved, String> {
    let mut resolved = Resolved::default();
    for path in &spec.write {
        push(&mut resolved.write, real(&checked(path)?));
    }
    for path in &spec.hidden {
        let given = checked(path)?;
        let real = real(&given);
        push(&mut resolved.hidden, given);
        push(&mut resolved.hidden, real);
    }
    Ok(resolved)
}

/// 没有的才加上。
fn push(out: &mut Vec<PathBuf>, path: PathBuf) {
    if !out.contains(&path) {
        out.push(path);
    }
}

/// 查一条路径：要是绝对路径，不能有 `.`、`..` 这样的段，不能有 NUL。交回整理过的写法（多出来的 `/` 去掉）。
///
/// `Path::components` 会悄悄吞掉中间的 `.`，所以照字节一段段看。
fn checked(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("path is not absolute: {path:?}"));
    }
    let bytes = path.as_os_str().as_encoded_bytes();
    if bytes
        .split(|&byte| byte == b'/')
        .any(|part| part == b"." || part == b"..")
    {
        return Err(format!("path has . or ..: {path:?}"));
    }
    if bytes.contains(&0) {
        return Err(format!("path has a NUL byte: {path:?}"));
    }
    Ok(path.components().collect())
}

/// 换成真实的位置：整条换（系统的 `realpath`）；换不了的（还不存在、没有权限），照最近一层换得了的上级换，再接上
/// 后面几段。一层都换不了的（不会有：根目录总在），照原样。
fn real(path: &Path) -> PathBuf {
    for base in path.ancestors() {
        let Ok(real) = std::fs::canonicalize(base) else {
            continue;
        };
        return match path.strip_prefix(base) {
            Ok(rest) if !rest.as_os_str().is_empty() => real.join(rest),
            _ => real,
        };
    }
    path.to_path_buf()
}

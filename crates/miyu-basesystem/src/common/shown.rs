//! 结果里的路径怎么写给她看（`10-自带软件.md` 第十节）：在这一轮的工作目录里的写相对路径，拿去就能直接给
//! `read`；在外面的写绝对路径。分隔符照平台原生的，Windows 上不写 `\\?\` 那个前缀。

use std::path::{Path, PathBuf};

use miyu_fs::resolve;
use miyu_tool::Call;

/// 照这一轮的工作目录写路径。
pub(crate) struct Shown {
    /// 工作目录换成的真实位置；换不成的是空的，路径一律写绝对的。
    cwd: Option<PathBuf>,
}

impl Shown {
    /// 照调用 `call` 的工作目录：和权限策略一样先换成真实的位置，头报来的可能是 `~`。
    pub(crate) fn here(call: &Call) -> Shown {
        Shown {
            cwd: resolve(Path::new(&call.cwd), call.home.as_deref(), &call.cwd).ok(),
        }
    }

    /// 真实的位置 `real` 写给她看的样子。
    pub(crate) fn path(&self, real: &Path) -> String {
        if let Some(cwd) = &self.cwd
            && let Ok(rest) = real.strip_prefix(cwd)
        {
            return if rest.as_os_str().is_empty() {
                ".".to_string()
            } else {
                rest.to_string_lossy().into_owned()
            };
        }
        plain(real)
    }
}

/// 绝对路径去掉 Windows 换真实位置时加上的 `\\?\`：`\\?\C:\x` 写成 `C:\x`，`\\?\UNC\host\share` 写成
/// `\\host\share`。别的平台上不会有这个前缀，照原样。
pub(super) fn plain(real: &Path) -> String {
    let text = real.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    match text.strip_prefix(r"\\?\") {
        Some(rest) => rest.to_string(),
        None => text.into_owned(),
    }
}

//! 人那一头（施工 8-3，`config.md` 第十条第 6、11 条）：`edit`、`trust` 要看是不是在终端里、问人一句、开编辑器。做成一个
//! 接口：真的一份照标准输入、标准错误和 `VISUAL`、`EDITOR`；测试换成照剧本回的，不用真的终端、真的编辑器。
//!
//! 编辑器照 `VISUAL`，没有照 `EDITOR`，都没有（或者是空的）的 Unix 上是 `vi`、Windows 上是 `notepad`。Unix 上经 `sh -c`
//! 跑，Windows 上经 `cmd /c`：带参数的（`code --wait`）也行，和 git 一样。

use std::io::{self, BufRead, IsTerminal};
use std::path::Path;
use std::process::Command;

/// 人那一头。
pub trait Console {
    /// 标准输入、标准错误都是终端：能问人、能开编辑器。
    fn terminal(&self) -> bool;
    /// 读人敲的一行，去掉换行；读到头了（管道关了、Ctrl+D）是空的。
    ///
    /// # Errors
    ///
    /// 读不了标准输入。
    fn line(&mut self) -> io::Result<Option<String>>;
    /// 用编辑器打开 `path`，等它退出，交回退出码；被信号停掉的没有退出码。
    ///
    /// # Errors
    ///
    /// 编辑器起不来。
    fn edit(&mut self, path: &Path) -> io::Result<Option<i32>>;
}

/// 真的终端：这个进程的标准输入、标准错误，和环境变量里的编辑器。
#[derive(Debug, Clone)]
pub struct Terminal {
    /// 编辑器的命令，照 shell 的写法。
    editor: String,
}

impl Terminal {
    /// 照这个进程的环境：`VISUAL`、`EDITOR`。
    pub fn current() -> Terminal {
        let var = |name: &str| std::env::var(name).ok();
        Terminal {
            editor: editor(var("VISUAL").as_deref(), var("EDITOR").as_deref()),
        }
    }
}

impl Console for Terminal {
    fn terminal(&self) -> bool {
        io::stdin().is_terminal() && io::stderr().is_terminal()
    }

    fn line(&mut self) -> io::Result<Option<String>> {
        let mut line = String::new();
        match io::stdin().lock().read_line(&mut line)? {
            0 => Ok(None),
            _ => Ok(Some(line.trim_end_matches(['\r', '\n']).to_string())),
        }
    }

    fn edit(&mut self, path: &Path) -> io::Result<Option<i32>> {
        editor_command(&self.editor, path)
            .status()
            .map(|status| status.code())
    }
}

/// 用哪个编辑器：`VISUAL`，没有照 `EDITOR`，都没有、是空的照系统的（Unix 上 `vi`，Windows 上 `notepad`）。
pub fn editor(visual: Option<&str>, editor: Option<&str>) -> String {
    match set(visual).or(set(editor)) {
        Some(chosen) => chosen.to_string(),
        None if cfg!(windows) => "notepad".to_string(),
        None => "vi".to_string(),
    }
}

/// 设了、去掉前后空白不是空的。
fn set(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// 用编辑器 `editor` 打开 `path` 的命令：Unix 上 `sh -c '<编辑器> "$@"' <编辑器> <文件>`，文件照一个参数交过去，不经 shell
/// 拆开；Windows 上 `cmd /c <编辑器> "<文件>"`。
pub fn editor_command(editor: &str, path: &Path) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new("cmd");
        command.raw_arg(format!("/c {editor} \"{}\"", path.display()));
        command
    }
    #[cfg(not(windows))]
    {
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(format!("{editor} \"$@\""))
            .arg(editor)
            .arg(path);
        command
    }
}

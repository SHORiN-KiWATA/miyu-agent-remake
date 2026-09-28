//! 用哪个 shell、怎么起（`10-自带软件.md` 第八节、B8，施工 4-8）：Linux 用 bash，macOS 用 zsh，Windows 用
//! PowerShell，装了 PowerShell 7 用它。核心起来时找一次，说明里写明是哪一种。
//!
//! 不读用户的启动文件：那里可能 `export` 了密钥，环境变量的白名单就白设了。`PATH` 照白名单从核心的环境带过去。
//! PowerShell 的命令编成 UTF-16LE 的 base64 传过去（`-EncodedCommand`），引号、换行不会被命令行拆坏。

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use base64::Engine;

#[cfg(test)]
mod tests;

/// 哪一种 shell。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// Linux。
    Bash,
    /// macOS 的默认 shell。
    Zsh,
    /// Windows 上装了的 PowerShell 7（`pwsh`）。
    PowerShell7,
    /// Windows 自带的 Windows PowerShell 5.1。
    WindowsPowerShell,
}

impl Kind {
    /// 写进说明里的名字：她照它写命令。PowerShell 的两种写法不一样（例如 5.1 没有 `&&`），名字里带上版本。
    pub(super) fn name(self) -> &'static str {
        match self {
            Kind::Bash => "bash",
            Kind::Zsh => "zsh",
            Kind::PowerShell7 => "PowerShell 7",
            Kind::WindowsPowerShell => "Windows PowerShell 5.1",
        }
    }
}

/// 这台机器上用的 shell：哪一种、程序在哪。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Program {
    /// 哪一种。
    pub(super) kind: Kind,
    /// 程序在哪。
    path: PathBuf,
}

/// PowerShell 执行她的命令之前先做的：输出编码设成不带 BOM 的 UTF-8（Windows PowerShell 默认用系统的代码页），关掉
/// 进度条（输出接到管道上时，进度条会变成一段 XML 写进标准错误）。和她的命令写在同一行，出错时报的行号还对得上。
const PRELUDE: &str = "[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); $ProgressPreference = 'SilentlyContinue'; ";

impl Program {
    /// 照这台机器找：`path` 是核心环境里的 `PATH`。
    pub(super) fn find(path: Option<&OsStr>) -> Program {
        choose(
            std::env::consts::OS,
            path,
            std::env::var_os("SystemRoot").as_deref(),
        )
    }

    /// 在工作目录 `cwd` 里执行 `script` 的命令，只带 `env` 里的环境变量。
    pub(super) fn command(
        &self,
        script: &str,
        cwd: &Path,
        env: Vec<(OsString, OsString)>,
    ) -> Command {
        let mut command = Command::new(&self.path);
        match self.kind {
            Kind::Bash => command.args(["--noprofile", "--norc", "-c", script]),
            // 没匹配到的通配符照原样传下去，和 bash 一样（施工 4-9 再补二）：zsh 默认直接报错。
            Kind::Zsh => command.args(["-f", "+o", "nomatch", "-c", script]),
            Kind::PowerShell7 | Kind::WindowsPowerShell => command
                .args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-EncodedCommand",
                ])
                .arg(encoded(&format!("{PRELUDE}{script}"))),
        };
        command.current_dir(cwd).env_clear().envs(env);
        command
    }
}

/// 照系统 `os`（`std::env::consts::OS` 的写法）挑：`path` 是 `PATH`，`system_root` 是 Windows 的 `SystemRoot`。
fn choose(os: &str, path: Option<&OsStr>, system_root: Option<&OsStr>) -> Program {
    let program = |kind, path| Program { kind, path };
    match os {
        "macos" => program(Kind::Zsh, PathBuf::from("/bin/zsh")),
        "windows" => match which(path, "pwsh.exe") {
            Some(found) => program(Kind::PowerShell7, found),
            None => program(
                Kind::WindowsPowerShell,
                system_root.map_or_else(
                    || PathBuf::from("powershell.exe"),
                    |root| {
                        Path::new(root)
                            .join("System32")
                            .join("WindowsPowerShell")
                            .join("v1.0")
                            .join("powershell.exe")
                    },
                ),
            ),
        },
        _ => program(
            Kind::Bash,
            which(path, "bash").unwrap_or_else(|| PathBuf::from("/bin/bash")),
        ),
    }
}

/// 在 `path` 列的目录里找叫 `name` 的程序。
fn which(path: Option<&OsStr>, name: &str) -> Option<PathBuf> {
    std::env::split_paths(path?)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// `-EncodedCommand` 要的写法：UTF-16LE 的 base64。
fn encoded(script: &str) -> String {
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

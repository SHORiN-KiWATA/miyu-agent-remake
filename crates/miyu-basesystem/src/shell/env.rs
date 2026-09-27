//! 命令拿得到的环境变量（`11-权限与沙盒.md` 第四节，施工 4-8）：只传白名单上的，其余一律不传。核心的环境里有从
//! 终端带来的密钥（例如 `DEEPSEEK_API_KEY`），命令拿不到。
//!
//! 第一版的名单是策略数据，写在这里，配置那一步能改（和 `miyu-fs` 里的边界清单一样）。本机的 ssh-agent
//! （`SSH_AUTH_SOCK`）和桌面（`DISPLAY`、`WAYLAND_DISPLAY`）有意不传：沙盒里的命令本来就连不上本机的套接字。

use std::ffi::{OsStr, OsString};

#[cfg(test)]
mod tests;

/// 照名字传的。
const PASSED: &[&str] = &[
    // 找程序、家目录、临时目录、用户。
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TMPDIR",
    "TMP",
    "TEMP",
    // 语言、时区、终端。
    "LANG",
    "LANGUAGE",
    "TZ",
    "TERM",
    // 工具链。
    "CARGO_HOME",
    "RUSTUP_HOME",
    "GOPATH",
    "GOROOT",
    "JAVA_HOME",
    "NVM_DIR",
    "PNPM_HOME",
    "VIRTUAL_ENV",
    // Windows 自己要的。
    "PATHEXT",
    "SystemRoot",
    "SystemDrive",
    "windir",
    "ComSpec",
    "USERPROFILE",
    "USERNAME",
    "HOMEDRIVE",
    "HOMEPATH",
    "APPDATA",
    "LOCALAPPDATA",
    "ProgramData",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "ProgramW6432",
    "CommonProgramFiles",
    "CommonProgramFiles(x86)",
    "CommonProgramW6432",
    "PUBLIC",
    "ALLUSERSPROFILE",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "OS",
    "PSModulePath",
];

/// 名字这样开头的都传：`LC_ALL`、`LC_CTYPE` 这些语言、地区的设置。
const PREFIXES: &[&str] = &["LC_"];

/// 另外设上的：git 要密码时直接失败，不去等人输入。名字都不在 [`PASSED`] 上，核心环境里原来的那一份挑不进来。
const SET: &[(&str, &str)] = &[("GIT_TERMINAL_PROMPT", "0")];

/// 从核心的环境 `vars` 里挑出命令拿得到的，再设上另外那几个。名字在 Windows 上不分大小写。
pub(super) fn passed(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
) -> Vec<(OsString, OsString)> {
    passed_as(vars, cfg!(windows))
}

/// 同 [`passed`]，`fold` 说名字比的时候分不分大小写：不分的是 Windows。
fn passed_as(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
    fold: bool,
) -> Vec<(OsString, OsString)> {
    let mut passed: Vec<(OsString, OsString)> = vars
        .into_iter()
        .filter(|(name, _)| allowed(name, fold))
        .collect();
    passed.extend(
        SET.iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value))),
    );
    passed
}

/// 名单上有没有 `name`。
fn allowed(name: &OsStr, fold: bool) -> bool {
    PASSED.iter().any(|passed| same(name, passed, fold))
        || name
            .to_str()
            .is_some_and(|name| PREFIXES.iter().any(|prefix| name.starts_with(prefix)))
}

/// 两个名字是不是同一个。
fn same(have: &OsStr, name: &str, fold: bool) -> bool {
    have.to_str().is_some_and(|have| match fold {
        true => have.eq_ignore_ascii_case(name),
        false => have == name,
    })
}

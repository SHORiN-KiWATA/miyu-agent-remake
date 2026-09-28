//! `miyu sandbox setup`、`miyu sandbox remove`（`docs/blueprint/sandbox/windows.md`，施工 5-8）：Windows 上装好、撤掉
//! 沙盒要管理员权限的那部分，一个专用的沙盒用户；别的平台上说一句不用装。
//!
//! - [`Sandbox`]：参数，连同只给提升过的自己用的两个（帮助里不列）；[`sandbox()`]：跑一次，交回退出码；
//! - 怎么走在 `flow.rs`，经「机器」这个接口干活：Windows 上是这里真的那一套，测试里换替身。
//!
//! 输出是给人看的：跟界面语言，成了印在标准输出上，没成印在标准错误上。一个字都不进模型面。

#[cfg(any(windows, test))]
mod flow;
#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use miyu_sandbox::install::InstallError;

use crate::ask::exit;
use crate::language::{self, Language};

/// `miyu sandbox` 的参数。给人看的说明在帮助页里（[`crate::help`]），这里的注释只给读代码的人看。
#[derive(Debug, Clone, Args)]
#[command(
    disable_help_subcommand = true,
    subcommand_required = true,
    arg_required_else_help = false
)]
pub struct Sandbox {
    /// 装还是卸。
    #[command(subcommand)]
    pub action: Action,
}

/// 装还是卸，连同只给提升过的自己用的那两个参数。
#[derive(Debug, Clone, Subcommand)]
pub enum Action {
    /// 建好沙盒用户。
    Setup(OwnerArgs),
    /// 撤掉沙盒用户。
    Remove(OwnerArgs),
}

/// 只给提升过的自己用：替谁装。两个一起写；帮助里不列。
#[derive(Debug, Clone, Args)]
pub struct OwnerArgs {
    /// 本人的数据根。
    #[arg(long, hide = true, requires = "owner_sid", value_name = "PATH")]
    pub owner_home: Option<PathBuf>,
    /// 本人的 SID。
    #[arg(long, hide = true, requires = "owner_home", value_name = "SID")]
    pub owner_sid: Option<String>,
}

impl Sandbox {
    /// 拆开：装还是卸，带没带替谁装的那两个参数。
    pub(crate) fn parts(self) -> (Which, Option<(PathBuf, String)>) {
        let (which, owner) = match self.action {
            Action::Setup(owner) => (Which::Setup, owner),
            Action::Remove(owner) => (Which::Remove, owner),
        };
        (which, owner.owner_home.zip(owner.owner_sid))
    }
}

/// 装还是卸，不带参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Which {
    /// `miyu sandbox setup`。
    Setup,
    /// `miyu sandbox remove`。
    Remove,
}

impl Which {
    /// 命令行上的写法。
    pub(crate) fn name(self) -> &'static str {
        match self {
            Which::Setup => "setup",
            Which::Remove => "remove",
        }
    }
}

/// 走完了要说的那一句。
#[derive(Debug)]
#[cfg_attr(
    not(any(windows, test)),
    expect(dead_code, reason = "要管理员、没成这两种只在 Windows 上走得到")
)]
pub(crate) enum Said {
    /// 装好了、撤干净了。
    Done(Which),
    /// 这个平台不用装。
    NotNeeded,
    /// 人取消了提升：要管理员权限。
    NeedsAdmin(Which),
    /// 没成：原因。
    Failed(Which, InstallError),
}

impl Said {
    /// 退出码：成了（连同不用装）是 0，别的是 1（`cli/main.md`「退出码」）。
    pub(crate) fn code(&self) -> u8 {
        match self {
            Said::Done(_) | Said::NotNeeded => exit::OK,
            Said::NeedsAdmin(_) | Said::Failed(..) => exit::ERROR,
        }
    }
}

/// 跑一次 `miyu sandbox setup` 或 `remove`：照界面语言说那一句，交回退出码。
pub fn sandbox(args: Sandbox) -> ExitCode {
    let language = language::current();
    let (which, given) = args.parts();
    let said = if miyu_sandbox::install::needed() {
        on_this_machine(which, given)
    } else {
        Said::NotNeeded
    };
    say(language, &said);
    ExitCode::from(said.code())
}

/// Windows：照 `flow.rs` 走，碰真的机器。
#[cfg(windows)]
fn on_this_machine(which: Which, given: Option<(PathBuf, String)>) -> Said {
    flow::run(&windows::Windows, which, given)
}

/// 别的平台上 `needed()` 是假的，走不到这里；还是照「不用装」说。
#[cfg(not(windows))]
fn on_this_machine(_which: Which, _given: Option<(PathBuf, String)>) -> Said {
    Said::NotNeeded
}

/// 说出来：成了的在标准输出上，没成的在标准错误上。
fn say(language: Language, said: &Said) {
    let line = language.sandbox(said);
    let written = match said.code() {
        exit::OK => writeln!(io::stdout().lock(), "{line}"),
        _ => writeln!(io::stderr().lock(), "{line}"),
    };
    #[expect(
        clippy::let_underscore_must_use,
        reason = "终端写不了的，没有别的地方可以说"
    )]
    let _ = written;
}

/// Windows 上真的那一套「机器」。
#[cfg(windows)]
mod windows {
    use std::ffi::OsString;
    use std::io;

    use miyu_sandbox::install::{self, Elevation, InstallError, Owner};
    use miyu_store::env::Env;
    use miyu_store::root::DataRoot;

    use super::Which;
    use super::flow::Machine;

    /// 这台 Windows。
    pub(super) struct Windows;

    impl Machine for Windows {
        fn elevated(&self) -> io::Result<bool> {
            install::elevated()
        }

        fn me(&self) -> Result<Owner, InstallError> {
            let failed =
                |detail: &dyn std::fmt::Display| InstallError::failed("find owner", detail);
            let root = DataRoot::locate(&Env::current()).map_err(|error| failed(&error))?;
            root.prepare().map_err(|error| failed(&error))?;
            let sid = miyu_pipe::current_user().map_err(|error| failed(&error))?;
            Owner::new(root.path().to_path_buf(), sid)
        }

        fn elevate(&self, args: &[OsString]) -> Result<u32, Elevation> {
            let program = std::env::current_exe().map_err(Elevation::Failed)?;
            install::elevate(&program, args)
        }

        fn apply(&self, which: Which, owner: &Owner) -> Result<(), InstallError> {
            match which {
                Which::Setup => install::setup(owner),
                Which::Remove => install::remove(owner),
            }
        }

        fn report(&self, owner: &Owner, error: &InstallError) -> Result<(), InstallError> {
            install::report(owner, error)
        }

        fn reported(&self, owner: &Owner) -> Result<Option<InstallError>, InstallError> {
            install::reported(owner)
        }

        fn clear(&self, owner: &Owner) -> Result<(), InstallError> {
            install::clear_report(owner)
        }
    }
}

//! `miyu web`（`docs/blueprint/web-module.md`「怎么走」第十一条第 1 款，施工 W-9）：主程序里不放网页的代码，照清单找网页
//! 软件（施工 9-3：子命令是 `web` 的那个包的程序，出厂那一份是 `miyu-web`），只找主程序真实位置旁边的（Windows 上加
//! `.exe`），把参数原样交给它的 `open`，等它退出，退出码照它的。没装的说怎么装，退出码 1。

#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::Args;

use miyu_kernel::id::AccountId;
use miyu_store::env::Env;
use miyu_store::packages::{Packages, locate};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::language::{self, Language};

/// 清单里找不到网页那一份时照它找：出厂的网页软件。
const PROGRAM: &str = "miyu-web";

/// `miyu web` 的参数：照原样交给网页软件。给人看的说明在帮助页里（[`crate::help`]）。
#[derive(Debug, Clone, Default, Args)]
pub struct Web {
    /// 网页软件这一次在哪个端口上听（没在跑时）。
    #[arg(long)]
    pub port: Option<u16>,
    /// 不开浏览器，印网址。
    #[arg(long)]
    pub print: bool,
    /// 忘了密码：带一次性码打开网页重设。
    #[arg(long)]
    pub reset: bool,
    /// 作废全部浏览器的登录。
    #[arg(long)]
    pub logout: bool,
    /// 打开时直接到「软件后台」里这个软件的页面（施工 F-6 下）。
    #[arg(long, value_name = "ID")]
    pub package: Option<String>,
}

impl Web {
    /// 交给 `miyu-web open` 的参数。
    fn args(&self) -> Vec<String> {
        let mut args = vec!["open".to_string()];
        if let Some(port) = self.port {
            args.extend(["--port".to_string(), port.to_string()]);
        }
        for (on, flag) in [
            (self.print, "--print"),
            (self.reset, "--reset"),
            (self.logout, "--logout"),
        ] {
            if on {
                args.push(flag.to_string());
            }
        }
        if let Some(package) = &self.package {
            args.extend(["--package".to_string(), package.clone()]);
        }
        args
    }
}

/// 跑一次 `miyu web`：照管理员 `admin` 那两层的清单找网页软件。
pub fn web(args: Web, admin: &AccountId) -> ExitCode {
    let main = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu"));
    let program = program(&Env::current(), admin);
    ExitCode::from(web_on(
        &args,
        &main,
        &program,
        language::current(),
        &mut io::stderr(),
    ))
}

/// 网页软件的程序名：两层清单里子命令是 `web` 的那一份的（施工 9-3）；没有的照出厂的 [`PROGRAM`]。
fn program(env: &Env, admin: &AccountId) -> String {
    let (Ok(root), Ok(resources)) = (DataRoot::locate(env), ResourceRoot::locate(env)) else {
        return PROGRAM.to_string();
    };
    Packages::new(&resources, &root, admin)
        .read()
        .into_iter()
        .filter_map(|found| found.read.ok()?.command)
        .find(|command| command.name == "web")
        .map_or_else(|| PROGRAM.to_string(), |command| command.program)
}

/// 同 [`web`]：主程序在 `main`，网页软件叫 `program`，没装时照 `language` 说在 `err` 上。交回退出码。
pub fn web_on(
    args: &Web,
    main: &Path,
    program: &str,
    language: Language,
    err: &mut dyn Write,
) -> u8 {
    let Some(program) = locate(program, main) else {
        if writeln!(err, "{}", not_installed(language)).is_err() {
            // 标准错误关了：没有别处可说。
        }
        return 1;
    };
    match Command::new(&program).args(args.args()).status() {
        Ok(status) => status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(1),
        Err(error) => {
            if writeln!(err, "miyu web: {}: {error}", program.display()).is_err() {
                // 标准错误关了：没有别处可说。
            }
            1
        }
    }
}

/// 没装网页界面：怎么装。
fn not_installed(language: Language) -> &'static str {
    match language {
        Language::Chinese => {
            "没装网页界面。装法：装和 miyu 同一个版本的 miyu-web 包（Arch：yay -S miyu-web；Debian、Ubuntu：apt install miyu-web；Fedora：dnf install miyu-web；macOS：brew install miyu-web），它装在 miyu 旁边。"
        }
        Language::English => {
            "The web UI is not installed. Install the miyu-web package of the same version as miyu (Arch: yay -S miyu-web; Debian, Ubuntu: apt install miyu-web; Fedora: dnf install miyu-web; macOS: brew install miyu-web); it goes next to miyu."
        }
    }
}

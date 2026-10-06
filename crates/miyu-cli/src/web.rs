//! `miyu web`（`docs/blueprint/web-module.md`「怎么走」第十一条第 1 款，施工 W-9）：主程序里不放网页的代码，只找主程序真实
//! 位置旁边的网页软件 `miyu-web`（Windows 上是 `miyu-web.exe`），把参数原样交给它的 `open`，等它退出，退出码照它的。没装的
//! 说怎么装，退出码 1。有了软件包的清单（M9）以后照清单找。

#[cfg(test)]
mod tests;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::Args;

use crate::language::{self, Language};

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
        args
    }
}

/// 跑一次 `miyu web`。
pub fn web(args: Web) -> ExitCode {
    let main = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu"));
    ExitCode::from(web_on(&args, &main, language::current(), &mut io::stderr()))
}

/// 同 [`web`]：主程序在 `main`，没装时照 `language` 说在 `err` 上。交回退出码。
pub fn web_on(args: &Web, main: &Path, language: Language, err: &mut dyn Write) -> u8 {
    let program = sibling(main, "miyu-web");
    if !program.is_file() {
        if writeln!(err, "{}", not_installed(language)).is_err() {
            // 标准错误关了：没有别处可说。
        }
        return 1;
    }
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

/// `program` 真实位置旁边叫 `name` 的程序（Windows 上加 `.exe`）。
pub fn sibling(program: &Path, name: &str) -> PathBuf {
    let real = std::fs::canonicalize(program).unwrap_or_else(|_| program.to_path_buf());
    let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    real.parent()
        .map_or_else(|| PathBuf::from(&file), |dir| dir.join(&file))
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

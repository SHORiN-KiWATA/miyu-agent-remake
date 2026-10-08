//! 软件包加的子命令（施工 9-2，`docs/blueprint/packages.md`「转交」，`cli/main.md`「分发」）：`miyu <名字> …` 不是内置的
//! 子命令时，照两层的清单找 `[command]` 是它的那个包，原样跑包里的程序：参数、环境、标准输入输出照原样，退出码照它的。
//! `miyu help <名字>` 转成 `<程序> --help`：帮助由包自己说。`miyu -h` 多一节「软件包加的命令」。
//!
//! 和内置的撞名：内置的优先，不转交、不列（装包时拦随装包那一步）。清单照磁盘现读，不连核心：转交不该为了找一个程序拉起
//! 核心。

#[cfg(test)]
mod tests;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::run;

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use miyu_config::phrases::Phrases;
use miyu_kernel::id::AccountId;
use miyu_store::env::Env;
use miyu_store::packages::{Packages, locate};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::language::Language;

/// 帮助页里命令那一列多宽（`help/<语言>/miyu.txt`：缩进两格，名字连空白到第 24 列）。
const NAME_COLUMN: usize = 22;

/// 一个包加的子命令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    /// `miyu` 后面敲的那个词。
    pub name: String,
    /// 跑哪个程序：程序名，不带路径。
    pub program: String,
    /// 一句说明，照语言挑。
    pub about: Phrases,
    /// 清单在哪：没装程序时说给人听。
    pub manifest: PathBuf,
}

/// 两层里读成了的、带 `[command]` 的清单加的子命令，照编号排。写错的、子命令名被先读到的占了的（`command_taken`）不算；
/// 名字是内置子命令 `builtins` 里的也不算：内置的优先。数据根、资源目录找不到的当没有。
pub fn added(env: &Env, admin: &AccountId, builtins: &[&str]) -> Vec<Added> {
    let (Ok(root), Ok(resources)) = (DataRoot::locate(env), ResourceRoot::locate(env)) else {
        return Vec::new();
    };
    Packages::new(&resources, &root, admin)
        .read()
        .into_iter()
        .filter_map(|found| {
            let manifest = found.read.ok()?;
            let command = manifest.command?;
            (!builtins.contains(&command.name.as_str())).then_some(Added {
                name: command.name,
                program: command.program,
                about: command.about,
                manifest: found.path,
            })
        })
        .collect()
}

/// 同 [`added`]，照这个进程的环境找数据根、资源目录。
pub fn installed(admin: &AccountId, builtins: &[&str]) -> Vec<Added> {
    added(&Env::current(), admin, builtins)
}

/// `miyu -h` 多的那一节：没有的是空的。名字照帮助页命令那一列对齐，说明照界面语言挑（挑法同 `package.list`）。
pub fn help_section(language: Language, added: &[Added]) -> String {
    if added.is_empty() {
        return String::new();
    }
    let title = match language {
        Language::Chinese => "软件包加的命令：",
        Language::English => "Commands from packages:",
    };
    let mut section = format!("{title}\n");
    for one in added {
        // 子命令名只有小写字母、数字、`-`（清单查过），一个字一列。
        let pad = NAME_COLUMN.saturating_sub(one.name.len()).max(2);
        let about = pick(&one.about, language).unwrap_or_default();
        section.push_str(&format!("  {}{}{about}\n", one.name, " ".repeat(pad)));
    }
    section
}

/// 帮助页 `page` 里，在「命令」那一节后面接上 `section`（空的照原样）：页面是用法一节、命令一节、再往下的几节，节和节之间
/// 空一行，接在第二个空行那里。
pub fn with_section(page: &str, section: &str) -> String {
    if section.is_empty() {
        return page.to_string();
    }
    let commands = page.find("\n\n").map(|at| at + 2);
    let end = commands.and_then(|start| page[start..].find("\n\n").map(|at| start + at));
    match end {
        Some(at) => format!("{}\n\n{}{}", &page[..at], section.trim_end(), &page[at..]),
        None => format!("{page}\n{section}"),
    }
}

/// 照语言挑一句：这种语言、`en`、`zh`、`ja` 的先后。
fn pick(phrases: &Phrases, language: Language) -> Option<String> {
    [language.code(), "en", "zh", "ja"]
        .iter()
        .find_map(|code| phrases.get(*code).cloned())
}

/// 该转交的交回退出码，不该的（第一个词是内置的、是选项、清单里没有）交回没有，照常解析。`args` 是整个命令行（第 0 个是
/// 主程序），主程序在 `main`；没装程序、跑不起来的照 `language` 在 `err` 上说。
pub fn forward(
    args: &[OsString],
    added: &[Added],
    main: &Path,
    language: Language,
    err: &mut dyn Write,
) -> Option<u8> {
    let first = args.get(1)?.to_str()?;
    let (name, rest): (&str, Vec<OsString>) = if first == "help" {
        let name = args.get(2)?.to_str()?;
        (name, vec![OsString::from("--help")])
    } else {
        (first, args[2..].to_vec())
    };
    let one = added.iter().find(|one| one.name == name)?;
    let Some(program) = locate(&one.program, main) else {
        say(err, &not_installed(one, language));
        return Some(1);
    };
    Some(run(&program, &rest, err))
}

/// 没装程序：哪一份清单说这个子命令由谁跑。
fn not_installed(one: &Added, language: Language) -> String {
    let manifest = one.manifest.display();
    match language {
        Language::Chinese => format!(
            "没找到 {}：清单 {manifest} 说 miyu {} 由它跑，miyu 旁边没有。装上这个包的程序再试。",
            one.program, one.name
        ),
        Language::English => format!(
            "{} not found: {manifest} says miyu {} runs it, but it is not next to miyu. Install the package's program and try again.",
            one.program, one.name
        ),
    }
}

fn say(err: &mut dyn Write, text: &str) {
    if writeln!(err, "{text}").is_err() {
        // 标准错误关了：没有别处可说。
    }
}

/// Unix 上换成它：信号、终端都直接到它，它的退出码就是这个进程的。换不成的说为什么，退出码 1。
#[cfg(unix)]
pub(crate) fn run(program: &Path, args: &[OsString], err: &mut dyn Write) -> u8 {
    use std::os::unix::process::CommandExt;
    let error = std::process::Command::new(program).args(args).exec();
    say(err, &format!("{}: {error}", program.display()));
    1
}

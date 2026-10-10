//! 打开默认的界面（施工 9-3，`docs/blueprint/cli/main.md`「怎么走」第 3 条，`packages.md`「入口」）：直接敲 `miyu`、`miyu
//! config` 时，连上核心（没在跑就拉起，界面本来就要它）问 `config.get` 拿 `ui.head`，照清单找这个编号的界面包，程序照
//! 9-2 的找法（只找 `miyu` 旁边的），换成它。`miyu config` 带 `--page config`，界面的清单 `[ui] opens` 里没有这一页的不拉起。
//! 只在标准输入、标准输出都是终端时才走到这里（主程序先看）。

#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::json;

use miyu_config::package::{Manifest, PackageKind};
use miyu_ipc::connect_or_start;
use miyu_kernel::id::AccountId;
use miyu_store::env::Env;
use miyu_store::packages::{Found, Packages, locate};
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;

use crate::language::{self, Language};
use crate::link;
use crate::packages;
use crate::rpc::Rpc;

/// 打开的结果。
#[derive(Debug, PartialEq, Eq)]
pub enum Opened {
    /// 拉起了（或者没拉起、说了为什么）：退出码。
    Ran(u8),
    /// 这个界面不认要的那一页：照旧印帮助。
    NoPage,
}

/// 打开 `ui.head` 指的界面，要的是 `page` 那一页（没有的是它的默认页）。管理员是 `admin`，拉起核心用 `start`。
pub fn open(page: Option<&str>, admin: &AccountId, start: impl FnOnce() -> Command) -> Opened {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            say(&error.to_string());
            return Opened::Ran(1);
        }
    };
    let env = Env::current();
    let asked = runtime.block_on(head(&env, start));
    drop(runtime);
    let (head, language) = match asked {
        Ok(asked) => asked,
        Err(reason) => {
            say(&reason);
            return Opened::Ran(1);
        }
    };
    let found = match (DataRoot::locate(&env), ResourceRoot::locate(&env)) {
        (Ok(root), Ok(resources)) => Packages::new(&resources, &root, admin).read(),
        _ => Vec::new(),
    };
    let main = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("miyu"));
    match plan(&head, page, &found, &main) {
        Plan::Run(program, args) => Opened::Ran(packages::run(&program, &args, &mut io::stderr())),
        Plan::NoPage => Opened::NoPage,
        Plan::Missing => {
            say(&unavailable(
                &head,
                None,
                &installed(&found, &main),
                language,
            ));
            Opened::Ran(1)
        }
        Plan::NoProgram(program) => {
            say(&unavailable(
                &head,
                Some(&program),
                &installed(&found, &main),
                language,
            ));
            Opened::Ran(1)
        }
    }
}

/// 连上核心，问 `ui.head`；交回它和这个连接说的语言。连不上、被拒、核心没答的交回原因。
async fn head(env: &Env, start: impl FnOnce() -> Command) -> Result<(String, Language), String> {
    let root = DataRoot::locate(env).map_err(|error| error.to_string())?;
    root.prepare().map_err(|error| error.to_string())?;
    let (connection, token) = connect_or_start(&root, start)
        .await
        .map_err(|error| error.to_string())?;
    let mut rpc = Rpc::new(connection, "head");
    let asked = language::current();
    let hello = link::hello(&mut rpc, &token, &asked, false, &mut io::stderr())
        .await
        .map_err(|_| String::new())?;
    let language = link::spoken(&hello, asked);
    let reply = rpc
        .call("config.get", json!({"keys": ["ui.head"]}))
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| language.disconnected())?;
    let head = reply["result"]["items"]["ui.head"]["value"]
        .as_str()
        .ok_or_else(|| reply.to_string())?;
    Ok((head.to_string(), language))
}

/// 照清单定怎么开。
#[derive(Debug, PartialEq, Eq)]
enum Plan {
    /// 跑这个程序，带这几个参数。
    Run(PathBuf, Vec<OsString>),
    /// 界面不认那一页。
    NoPage,
    /// 没有这个界面：没有清单、不是界面、没有子命令。
    Missing,
    /// 有清单，程序不在 `miyu` 旁边：程序名（9-3 补：出厂带了终端的清单，程序随 M9）。
    NoProgram(String),
}

/// 编号是 `head` 的界面包怎么开：要 `page` 那一页的带 `--page <页>`，清单里没认这一页的不开。
fn plan(head: &str, page: Option<&str>, found: &[Found], main: &Path) -> Plan {
    let Some(manifest) = found
        .iter()
        .find(|one| one.id == head)
        .and_then(|one| one.read.as_ref().ok())
        .filter(|manifest| manifest.kind == PackageKind::Ui)
    else {
        return Plan::Missing;
    };
    if let Some(page) = page
        && !opens(manifest, page)
    {
        return Plan::NoPage;
    }
    let Some(command) = manifest.command.as_ref() else {
        return Plan::Missing;
    };
    let Some(program) = locate(&command.program, main) else {
        return Plan::NoProgram(command.program.clone());
    };
    let args = page.map_or_else(Vec::new, |page| {
        vec![OsString::from("--page"), OsString::from(page)]
    });
    Plan::Run(program, args)
}

/// 界面的清单认不认 `page` 这一页。
fn opens(manifest: &Manifest, page: &str) -> bool {
    manifest
        .ui
        .as_ref()
        .is_some_and(|ui| ui.opens.iter().any(|one| one == page))
}

/// 装了的界面：读成了的、`kind = "ui"`、有子命令、程序在 `miyu`（`main`）旁边的，照编号排。只有清单、程序还不在的不算：
/// 列出来也打不开（9-3 补）。
fn installed(found: &[Found], main: &Path) -> Vec<String> {
    found
        .iter()
        .filter(|one| {
            one.read.as_ref().is_ok_and(|manifest| {
                manifest.kind == PackageKind::Ui
                    && manifest
                        .command
                        .as_ref()
                        .is_some_and(|command| locate(&command.program, main).is_some())
            })
        })
        .map(|one| one.id.clone())
        .collect()
}

/// 打不开 `head`：为什么（没有这个界面；有清单、程序 `program` 不在 `miyu` 旁边），怎么办，装了的有哪几个。
fn unavailable(
    head: &str,
    program: Option<&str>,
    installed: &[String],
    language: Language,
) -> String {
    let (why, fix) = match (language, program) {
        (Language::Chinese, None) => (
            format!("没装 {head} 这个界面（ui.head 指着它）。"),
            "装上它的软件包",
        ),
        (Language::Chinese, Some(program)) => (
            format!("{head} 这个界面的程序 {program} 不在 miyu 旁边（ui.head 指着它）。"),
            "把它放到 miyu 旁边",
        ),
        (Language::English, None) => (
            format!("The {head} interface is not installed (ui.head names it). "),
            "Install its package",
        ),
        (Language::English, Some(program)) => (
            format!(
                "The {head} interface's program {program} is not next to miyu (ui.head names it). "
            ),
            "Put it next to miyu",
        ),
    };
    let next = match (language, installed.is_empty()) {
        (Language::Chinese, true) => format!(
            "{fix}，或者 miyu config set ui.head <编号> 换成装了的界面；现在一个界面都没装，可以先用 miyu ask \"…\" 和 AI 对话。"
        ),
        (Language::Chinese, false) => format!(
            "{fix}，或者 miyu config set ui.head <编号> 换成装了的界面：{}。",
            installed.join("、")
        ),
        (Language::English, true) => format!(
            "{fix}, or switch with miyu config set ui.head <id>; no interface is installed yet, so talk to the AI with miyu ask \"…\" for now."
        ),
        (Language::English, false) => format!(
            "{fix}, or switch with miyu config set ui.head <id> to one that is installed: {}.",
            installed.join(", ")
        ),
    };
    format!("{why}{next}")
}

fn say(text: &str) {
    if !text.is_empty() && writeln!(io::stderr(), "{text}").is_err() {
        // 标准错误关了：没有别处可说。
    }
}

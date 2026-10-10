//! 装、卸之前印的那一份和问的那一句（施工 F-8 下补，`docs/blueprint/cli/pkg.md`「怎么走」第 3 步，照 pacman）：核心看一眼的
//! 回应（`package.install`、`package.remove` 带 `preview`）印成几行，空一行问「继续？[Y/n]」，读一行答：空的、`y`、`yes`
//! 接着做，别的不做；标准输入关着的（脚本里）也不做，提示用 `--yes`。

use std::io::{BufRead, Write};

use serde_json::Value;

use super::query::size;
use crate::exit;
use crate::language::{Carried, Doing, Language, PlanLabel};
use crate::shown::{say, write};

/// 看一眼的回应 `preview` 印成几行：哪个包（版本、换下哪一份）、包含什么、需要什么能力、一并删除什么、多大。没有的那一行不印。
pub(crate) fn plan(preview: &Value, doing: Doing, language: &Language) -> Vec<String> {
    let separator = language.list_separator();
    let mut head = language.will(doing, preview["package"].as_str().unwrap_or_default());
    if let Some(version) = preview["version"].as_str() {
        head.push(' ');
        head.push_str(version);
    }
    if let Some(replaces) = preview.get("replaces") {
        head.push_str(&language.replacing(replaces["version"].as_str()));
    }
    let mut rows = vec![head];
    let mut row = |label: PlanLabel, items: Vec<String>| {
        if !items.is_empty() {
            rows.push(format!(
                "{}{}",
                language.plan_label(label),
                items.join(separator)
            ));
        }
    };
    row(PlanLabel::Includes, included(preview, doing, language));
    let needs = preview["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|capability| capability["name"].as_str())
        .map(str::to_string)
        .collect();
    row(PlanLabel::Needs, needs);
    if doing == Doing::Remove {
        let keys: Vec<&str> = strings(&preview["settings"]);
        let mut deletes = Vec::new();
        if !keys.is_empty() {
            deletes.push(language.deleted_settings(&keys.join(separator)));
        }
        if preview["state"] == true {
            deletes.push(language.state_dir().to_string());
        }
        row(PlanLabel::AlsoDeletes, deletes);
    }
    if let (Some(files), Some(bytes)) = (preview["files"].as_u64(), preview["size"].as_u64()) {
        row(
            PlanLabel::Size,
            vec![language.package_size(&size(bytes), files)],
        );
    }
    rows
}

/// 「包含」那一行：程序、子命令、后台页、吉祥物、接的平台、系统账号、几项设置（卸的时候设置算在「一并删除」里）。
fn included(preview: &Value, doing: Doing, language: &Language) -> Vec<String> {
    let mut items = Vec::new();
    if let Some(kind) = preview["program"].as_str() {
        items.push(language.program(kind));
    }
    let mut carried = |what: Carried<'_>| items.push(language.carried(what));
    if let Some(name) = preview["command"].as_str() {
        carried(Carried::Command(name));
    }
    if preview["page"] == true {
        carried(Carried::Page);
    }
    if preview["mascot"] == true {
        carried(Carried::Mascot);
    }
    if let Some(platform) = preview["connection"].as_str() {
        carried(Carried::Connection(platform));
    }
    if preview["system_account"] == true {
        carried(Carried::SystemAccount);
    }
    if let Some(n) = preview["settings"]
        .as_u64()
        .filter(|_| doing != Doing::Remove)
    {
        carried(Carried::Settings(n));
    }
    items
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect()
}

/// 空一行问一句、读一行答。接着做的 `Ok`；不做的在标准错误上说一句，交回退出码 1。
pub(crate) fn asked(
    input: &mut dyn BufRead,
    out: &mut dyn Write,
    err: &mut dyn Write,
    doing: Doing,
    language: &Language,
) -> Result<(), u8> {
    write(out, &format!("\n{}", language.proceed(doing)));
    let mut answer = String::new();
    match input.read_line(&mut answer) {
        Ok(0) | Err(_) => {
            say(out, "");
            say(err, language.unconfirmed());
            Err(exit::ERROR)
        }
        Ok(_) => match answer.trim().to_lowercase().as_str() {
            "" | "y" | "yes" => Ok(()),
            _ => {
                say(err, language.cancelled());
                Err(exit::ERROR)
            }
        },
    }
}

#[cfg(test)]
mod tests;

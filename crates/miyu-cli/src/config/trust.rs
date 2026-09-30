//! `miyu config trust`（`config.md` 第十条第 11 条，施工 8-3）：看当前目录的项目配置会改什么，信任或者不信任它。
//!
//! 1. `config.get`，带当前目录当 `cwd`、`all`。没有项目配置：说一句，退出码 1。
//! 2. 标准输出上印出这份文件在哪、它会改哪几项（`键 = 值` 和名字，几列照显示的宽度对齐），有问题的照报错一行印在后面。
//! 3. 已经信任着这一份、又没写 `--no`：说一句，退出码 0。
//! 4. 写了 `--yes`、`--no` 的照它，不问。都没写：不在终端里的说一句，退出码 2；在终端里的问一句 `[y/N]`，`y`、`yes`（不分
//!    大小写）是信任，别的、直接回车是不信任，都记下。
//! 5. `config.trust` 带第 1 步读到的版本。别处又改过了：说一句，退出码 1。成了：印一行灰字，退出码 0。

use std::io::Write;

use serde_json::{Value, json};

use super::console::Console;
use super::paths::Places;
use super::render::{self, columns, toml};
use super::{MISUSE, Reply, Talk};
use crate::exit;
use crate::shown::{Line, say, write};

/// 信不信任，交回退出码。`answer` 是 `--yes`（`Some(true)`）、`--no`（`Some(false)`），都没写的是空的。
pub(super) async fn trust(
    talk: &mut Talk<'_>,
    answer: Option<bool>,
    console: &mut dyn Console,
    out: &mut dyn Write,
) -> u8 {
    let language = talk.plan.language;
    let params = json!({"cwd": talk.cwd(), "all": true});
    let got = match talk.ask("config.get", params).await {
        Ok(result) => result,
        Err(code) => return code,
    };
    let project = &got["files"]["project"];
    let Some(file) = project["file"].as_str() else {
        say(talk.err, language.no_project_here());
        return exit::ERROR;
    };
    let schema = match talk.ask("config.schema", json!({})).await {
        Ok(result) => result,
        Err(code) => return code,
    };
    let shown = Places::of(talk.plan).shown(file);
    say(out, &language.would_set(&shown));
    for line in rows(&got["items"], &schema["items"]) {
        say(out, &line);
    }
    for problem in got["problems"].as_array().into_iter().flatten() {
        if problem["file"] == file && problem["code"] != "untrusted_project" {
            let line = render::problem(&shown, problem, language);
            write(out, &line.paint(talk.plan.color));
        }
    }
    if project["trusted"] == true && answer != Some(false) {
        say(talk.err, language.already_trusted());
        return exit::OK;
    }
    let trusted = match answer {
        Some(answer) => answer,
        None if !console.terminal() => {
            say(talk.err, language.trust_needs_terminal());
            return MISUSE;
        }
        None => {
            write(talk.err, language.trust_question());
            let said = console.line().ok().flatten().unwrap_or_default();
            matches!(said.trim().to_lowercase().as_str(), "y" | "yes")
        }
    };
    let params = json!({"cwd": talk.cwd(), "version": project["version"], "trust": trusted});
    match talk.request("config.trust", params).await {
        Ok(Reply::Done(_)) => {
            let line = Line::gray(language.trust_answered(trusted));
            write(talk.err, &line.paint(talk.plan.gray));
            exit::OK
        }
        Ok(Reply::Refused(error)) if error["data"]["reason"] == "config_conflict" => {
            say(talk.err, language.trust_conflict());
            exit::ERROR
        }
        Ok(Reply::Refused(error)) => talk.refused(&error),
        Err(code) => code,
    }
}

/// 项目配置会改的每一项一行：`  键 = 值  名字`，`键 = 值` 那一列补齐到最宽的。
fn rows(items: &Value, schema: &Value) -> Vec<String> {
    let mut rows = Vec::new();
    for (key, item) in items.as_object().into_iter().flatten() {
        // 信任了才算的那几项：还没信任的算，不能写、写宽了的不算，照报错一行印在后面。
        let written = item["layers"].as_array().into_iter().flatten().find(|row| {
            row["origin"]["layer"] == "project"
                && (row["problem"].is_null() || row["problem"] == "untrusted_project")
        });
        if let Some(row) = written {
            let name = schema
                .as_array()
                .into_iter()
                .flatten()
                .find(|said| said["key"] == key.as_str())
                .and_then(|said| said["name"].as_str())
                .unwrap_or_default();
            rows.push((format!("{key} = {}", toml(&row["value"])), name.to_string()));
        }
    }
    let width = rows.iter().map(|(set, _)| columns(set)).max().unwrap_or(0);
    rows.into_iter()
        .map(|(set, name)| {
            let pad = " ".repeat(width.saturating_sub(columns(&set)));
            format!("  {set}{pad}  {name}")
        })
        .collect()
}

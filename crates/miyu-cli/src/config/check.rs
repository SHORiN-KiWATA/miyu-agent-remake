//! `miyu config check [文件]`（`config.md` 第十条第 7 条）：查配置有没有写错。
//!
//! - 不写文件：照 `config.get` 的 `files` 读系统配置、个人设置、当前目录的项目配置现在的字（磁盘上的，不是核心手里的
//!   那份），一份份 `config.check`。写了 `--system`、`--project` 的只查那一份。
//! - 写了文件：照 `--system`、`--project` 当那一层查，都不写的当个人设置。
//! - 标准输出上一条一行，最后一行合计；一个问题都没有的印「没有问题」。`--format json`：`{"problems":[…]}`，每一条
//!   多一格 `file`。有错误退出码 1，只有警告、没有问题的 0。
//!
//! 命令行自己读不了的文件（读不了、太大、不是 UTF-8）照核心的说法报一条，不交给核心。

use std::io::Write;
use std::path::Path;

use serde_json::{Value, json};

use miyu_store::config_file::{self, ReadError};

use super::Talk;
use super::paths::Places;
use super::render;
use crate::ask::Format;
use crate::exit;
use crate::shown::{Ink, Line, say, write};

/// 只查哪一份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Only {
    /// 都查；写了文件的当个人设置。
    All,
    /// 系统配置。
    System,
    /// 项目配置。
    Project,
}

impl Only {
    /// 照两个选项：`--system`、`--project` 最多写一个（clap 拦着）。
    pub(super) fn of(system: bool, project: bool) -> Only {
        match (system, project) {
            (true, _) => Only::System,
            (_, true) => Only::Project,
            _ => Only::All,
        }
    }
}

/// 查一遍，印出来，交回退出码。
pub(super) async fn check(
    talk: &mut Talk<'_>,
    file: Option<&Path>,
    only: Only,
    format: Format,
    out: &mut dyn Write,
) -> u8 {
    let places = Places::of(talk.plan);
    let mut targets: Vec<(&str, String, std::path::PathBuf)> = Vec::new();
    match file {
        Some(file) => {
            let layer = match only {
                Only::System => "system",
                Only::Project => "project",
                Only::All => "personal",
            };
            let shown = crate::shown::tilde(&file.to_string_lossy(), talk.plan.home.as_deref());
            targets.push((layer, shown, file.to_path_buf()));
        }
        None => {
            let files = match talk.ask("config.get", json!({"cwd": talk.cwd()})).await {
                Ok(result) => result["files"].clone(),
                Err(code) => return code,
            };
            for layer in ["system", "personal", "project"] {
                let wanted = match only {
                    Only::All => true,
                    Only::System => layer == "system",
                    Only::Project => layer == "project",
                };
                if let (true, Some(file)) = (wanted, files[layer]["file"].as_str()) {
                    targets.push((layer, places.shown(file), places.absolute(file)));
                }
            }
        }
    }
    let mut problems: Vec<Value> = Vec::new();
    for (layer, shown, path) in targets {
        let text = match config_file::read(&path) {
            Ok(Some(text)) => text.text,
            // 不写文件时，还没有的那一层没什么可查；写了文件的，没有就是读不了。
            Ok(None) if file.is_none() => continue,
            Ok(None) => {
                let missing = std::io::Error::from(std::io::ErrorKind::NotFound);
                problems.push(unreadable(talk, &shown, &ReadError::Unreadable(missing)));
                continue;
            }
            Err(error) => {
                problems.push(unreadable(talk, &shown, &error));
                continue;
            }
        };
        let result = match talk
            .ask("config.check", json!({"layer": layer, "text": text}))
            .await
        {
            Ok(result) => result,
            Err(code) => return code,
        };
        for mut problem in result["problems"].as_array().cloned().unwrap_or_default() {
            problem["file"] = json!(shown);
            problems.push(problem);
        }
    }
    let errors = problems
        .iter()
        .filter(|problem| problem["level"] != "warning")
        .count();
    match format {
        Format::Json => say(out, &json!({ "problems": problems }).to_string()),
        Format::Text => {
            let language = talk.plan.language;
            for problem in &problems {
                let file = problem["file"].as_str().unwrap_or_default();
                write(
                    out,
                    &render::problem(file, problem, language).paint(talk.plan.color),
                );
            }
            let total = language.problems_total(errors, problems.len() - errors);
            write(out, &Line::inked(Ink::Plain, total).paint(talk.plan.color));
        }
    }
    match errors {
        0 => exit::OK,
        _ => exit::ERROR,
    }
}

/// 命令行自己读不了的一份：照核心的说法写成一条问题。
fn unreadable(talk: &Talk<'_>, shown: &str, error: &ReadError) -> Value {
    let language = talk.plan.language;
    let (code, message) = match error {
        ReadError::Unreadable(why) => ("unreadable", language.unreadable_config(&why.to_string())),
        ReadError::TooBig => ("too_big", language.config_too_big().to_string()),
        ReadError::NotUtf8 => ("not_utf8", language.config_not_utf8().to_string()),
    };
    json!({"code": code, "file": shown, "level": "error", "message": message})
}

//! `miyu check [文件]`（施工 8-30，`docs/blueprint/cli/check.md`）：查人手写的、Miyu 读的文件有没有写错。核心照磁盘上现在的
//! 字查（`check` 方法：配置、密钥文件、人格），这里只印。
//!
//! - 标准输出上一处一行 `<文件>:<行>:<列> <级别>：<那一句>`，最后一行合计；一处都没有的印「没有问题」。`--format json`：
//!   核心的回应原样，`{"problems":[…]}`。有错误退出码 1，只有警告、没有问题的 0。
//! - 写了文件的只查那一份；核心认不出是哪一种的照它的原话报（`unknown_file`），退出码 1。

use std::io::Write;
use std::path::Path;

use serde_json::json;

use super::Talk;
use super::paths::Places;
use super::render;
use crate::ask::Format;
use crate::exit;
use crate::shown::{Ink, Line, say, write};

/// 查一遍，印出来，交回退出码。
pub(super) async fn check(
    talk: &mut Talk<'_>,
    file: Option<&Path>,
    format: Format,
    out: &mut dyn Write,
) -> u8 {
    let mut params = json!({"cwd": talk.cwd()});
    if let Some(file) = file {
        params["file"] = json!(file.to_string_lossy());
    }
    let result = match talk.ask("check", params).await {
        Ok(result) => result,
        Err(code) => return code,
    };
    let problems = result["problems"].as_array().cloned().unwrap_or_default();
    let errors = problems
        .iter()
        .filter(|problem| problem["level"] != "warning")
        .count();
    match format {
        Format::Json => say(out, &json!({ "problems": problems }).to_string()),
        Format::Text => {
            let places = Places::of(talk.plan);
            let language = talk.plan.language;
            for problem in &problems {
                let file = places.shown(problem["file"].as_str().unwrap_or_default());
                write(
                    out,
                    &render::problem(&file, problem, language).paint(talk.plan.color),
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

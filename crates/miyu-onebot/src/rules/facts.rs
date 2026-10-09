//! 给她看的事实的模板（施工 O-25 下，`onebot.md` 第一条「退信」第 3 条、「场所规则和出厂数据」第 1、2 条）：资源
//! `software/onebot/facts/undelivered.txt`（给模型看的字，登记在 `26-提示词.md` 第十节），桥起来时和别的出厂数据一起读，交内核的
//! [`Template::parse`] 查。没有系统那一份：给模型看的字随包走。
//!
//! 字段只认 [`FIELDS`]：多要了别的，填的时候换不出来，起来时就当打包的错（「施工时定的」第 129 条）。少要几个照收。

use std::path::Path;

use miyu_chat::{Problem, Source};
use miyu_config::problem::Code;
use miyu_kernel::template::Template;

use super::{files, required};

/// 退信的模板在出厂目录里的位置；问题里的文件名也照它写。
const UNDELIVERED: &str = "facts/undelivered.txt";

/// 退信的模板认的字段：为什么、NapCat 说的原因、她那一段的开头（「退信」第 3 条）。
const FIELDS: [&str; 3] = ["why", "detail", "text"];

/// 读出厂目录 `dir`（`software/onebot/`）里退信的模板，查过交回。不在、读不成的记一条问题；写坏了、要了 [`FIELDS`] 以外的字段
/// 的记一条 `bad_format`，原话说错在哪。有问题的交回空的。
pub(super) fn undelivered(dir: &Path, problems: &mut Vec<Problem>) -> Option<Template> {
    let text = required(&dir.join(UNDELIVERED), UNDELIVERED, problems)?;
    let checked = Template::parse(&text)
        .map_err(|error| error.to_string())
        .and_then(|template| {
            match template
                .fields()
                .iter()
                .find(|field| !FIELDS.contains(field))
            {
                Some(field) => Err(format!("unknown field {{{field}}}: only {FIELDS:?}")),
                None => Ok(template),
            }
        });
    match checked {
        Ok(template) => Some(template),
        Err(why) => {
            problems.push(files::whole(
                Code::BadFormat,
                Source::Factory,
                UNDELIVERED,
                Some(why),
            ));
            None
        }
    }
}

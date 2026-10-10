//! 桥的工具（施工 O-26，`onebot.md` 第一条「提供者和不说话」，「场所规则和出厂数据」第 1、2 条）：桥答得了哪几件、各自的访问
//! 类别和给哪种会话写在 [`TOOLS`] 这张表里（跟着答它的代码，「施工时定的」第 138 条）；说明照资源
//! `software/onebot/tools/<名字>.json` 读（`miyu_tool::load::spec`，`providers.md`「给模型看的字」），答的两句照
//! `software/onebot/tool-results/` 读（`miyu_tool::load::text`，试换过字段）。都是给模型看的字，登记在 `26-提示词.md` 第十节；
//! 桥起来时和别的出厂数据一起读，写坏了是打包的错，起不来。
//!
//! 交出去的两样：`provide` 的参数（[`Tools::provided`]），`tool.call` 的结果（[`Tools::call`]）。

use std::path::Path;

use miyu_chat::{Problem, Source};
use miyu_config::problem::Code;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::Spec;
use miyu_tool::load::{self, LoadError};
use serde_json::{Value, json};

use super::files;
use crate::PACKAGE;

/// 「这一轮不说话」（「提供者和不说话」第 3 条）：她的回复里有这件的调用块，这一轮就不发了（`core/route/quiet.rs`）。
pub(crate) const SKIP_REPLY: &str = "skip_reply";

/// 桥答得了的工具：名字、访问类别、给哪种会话（`providers.md` 的 `venues`）。`skip_reply` 只读（施工单「要定的」第 4 条：不碰
/// 主机、不往外发），给私聊、群，本机的会话没有。
const TOOLS: [(&str, Access, &[&str]); 1] = [(SKIP_REPLY, Access::Read, &["private", "group"])];

/// 答的那几句在 `software/onebot/` 下的哪个目录里（照核心的 `core/tool-results/`）。
const RESULTS: &str = "tool-results";

/// 答 `skip_reply` 的那一句：不要字段。
const SKIPPED: &str = "skipped";

/// 不认识的工具答的那一句：字段只认 `name`（工具名）。
const UNKNOWN: &str = "unknown";

/// 读好、查过的桥的工具。
#[derive(Debug)]
pub struct Tools {
    /// 每件的规格和给哪种会话，照 [`TOOLS`] 的先后。
    specs: Vec<(Spec, &'static [&'static str])>,
    /// 答 `skip_reply` 的那一句（`tool-results/skipped.txt`）。
    skipped: String,
    /// 不认识的工具答的那一句（`tool-results/unknown.txt`）。
    unknown: Template,
}

impl Tools {
    /// `provide` 的参数（「提供者和不说话」第 1 条）：每件 `{name, description, input_schema, access, venues}`，不写
    /// `timeout_ms`（照核心的一分钟：这几件当场答）。
    pub(crate) fn provided(&self) -> Value {
        let tools: Vec<Value> = self
            .specs
            .iter()
            .map(|(spec, venues)| {
                json!({
                    "name": spec.name,
                    "description": spec.description,
                    "input_schema": spec.parameters,
                    "access": spec.access,
                    "venues": venues,
                })
            })
            .collect();
        json!({"tools": tools})
    }

    /// 核心调工具 `tool` 的结果（`tool.call` 回应的 `result`，「提供者和不说话」第 2、3 条）：`skip_reply` 答出厂的那一句，
    /// 不算出错；不认识的答 `unknown.txt` 填上那个名字，算出错。
    pub(crate) fn call(&self, tool: &str) -> Value {
        if tool == SKIP_REPLY {
            return result(&self.skipped, false);
        }
        // 起来时试换过 `name`：换得出。
        result(&load::say(&self.unknown, &[("name", tool)]), true)
    }
}

/// 一段字的结果：内容块照内核的写法，`error` 照写。
fn result(text: &str, error: bool) -> Value {
    json!({"blocks": [{"type": "text", "text": text}], "error": error})
}

/// 读资源目录 `resources` 里桥的工具：说明、答的两句。读不成、写坏了、要了别的字段的，每一份记一条 `bad_format`（文件照
/// `software/onebot/` 下的位置写，原话照 `miyu_tool::load` 说的），交回空的。
pub(super) fn load(resources: &Path, problems: &mut Vec<Problem>) -> Option<Tools> {
    let mut trouble = |file: String, error: LoadError| {
        problems.push(files::whole(
            Code::BadFormat,
            Source::Factory,
            &file,
            Some(error.why),
        ));
    };
    let mut specs = Vec::new();
    for (name, access, venues) in &TOOLS {
        match load::spec(resources, PACKAGE, name, access.clone()) {
            Ok(spec) => specs.push((spec, *venues)),
            Err(error) => trouble(format!("tools/{name}.json"), error),
        }
    }
    let text = |name: &str, fields: &[&str]| {
        load::text(resources, PACKAGE, RESULTS, name, fields)
            .map_err(|error| (format!("{RESULTS}/{name}.txt"), error))
    };
    let skipped = text(SKIPPED, &[]).map(|template| load::say(&template, &[]));
    let unknown = text(UNKNOWN, &["name"]);
    match (skipped, unknown) {
        (Ok(skipped), Ok(unknown)) if specs.len() == TOOLS.len() => Some(Tools {
            specs,
            skipped,
            unknown,
        }),
        (skipped, unknown) => {
            for (file, error) in [skipped.err(), unknown.err()].into_iter().flatten() {
                trouble(file, error);
            }
            None
        }
    }
}

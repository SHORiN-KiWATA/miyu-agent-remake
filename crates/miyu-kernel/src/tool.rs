//! 内核眼里的工具（`docs/designs/05-内核接口.md` 第六节）：访问类别、参数格式；执行之前的
//! 参数修正；内核自己拦下时写给模型的那两句（`02-内核.md` 第六节「工具怎么调、下一步怎么走」）。
//!
//! 工具由软件包提供，内核不内置（`10-自带软件.md` 第一节）。内核只要知道两样：能不能和别的
//! 一起跑（看访问类别），参数长什么样（修正畸形参数）。

use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::raw::RawJson;
use crate::template::{Template, TemplateError};

/// 工具的访问类别。权限策略、能不能一起跑、撤销前要不要存档，都看它。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// 只读：可以和别的只读调用一起跑。
    Read,
    /// 写文件。
    Write,
    /// 执行命令。
    Execute,
    /// 访问网络。
    Network,
    /// 对外发消息。
    Outbound,
}

/// 内核要知道的一件工具：访问类别和参数格式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRule {
    /// 访问类别。
    pub access: Access,
    /// 参数的 JSON Schema，和工具面上的一样。
    pub parameters: RawJson,
}

/// 参数不是一个 JSON 对象，修正不了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotAnObject;

/// 照参数格式修正模型给的参数原文，返回交给执行的参数：一个 JSON 对象的原文。
///
/// 只动参数格式里声明了类型的顶层参数：被写成字符串的数组、对象、整数、数字、布尔，
/// 能还原成那个类型才换；声明成字符串的，一个字节都不碰。什么都没改的，原文照交。
/// 什么都没写的，当成空对象：有的供应商给没有参数的调用发空字符串。
///
/// # Errors
///
/// 参数不是一个 JSON 对象，返回 [`NotAnObject`]。
pub fn repair(parameters: &RawJson, args: &str) -> Result<String, NotAnObject> {
    if args.trim().is_empty() {
        return Ok("{}".to_string());
    }
    let mut object: Map<String, Value> = serde_json::from_str(args).map_err(|_| NotAnObject)?;
    let schema: Value = match serde_json::from_str(parameters.get()) {
        Ok(schema) => schema,
        Err(_) => return Ok(args.to_string()),
    };
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Ok(args.to_string());
    };
    let mut repaired = false;
    for (name, declared) in properties {
        let Some(kind) = declared.get("type").and_then(Value::as_str) else {
            continue;
        };
        let Some(Value::String(text)) = object.get(name) else {
            continue;
        };
        if let Some(value) = restore(kind, text.trim()) {
            object.insert(name.clone(), value);
            repaired = true;
        }
    }
    if repaired {
        Ok(Value::Object(object).to_string())
    } else {
        Ok(args.to_string())
    }
}

/// 一段字能不能还原成声明的类型。布尔大小写都收：模型发过 Python 风格的 `"False"`。
fn restore(kind: &str, text: &str) -> Option<Value> {
    match kind {
        "array" if text.starts_with('[') => serde_json::from_str::<Value>(text)
            .ok()
            .filter(Value::is_array),
        "object" if text.starts_with('{') => serde_json::from_str::<Value>(text)
            .ok()
            .filter(Value::is_object),
        "integer" => text.parse::<i64>().ok().map(Value::from),
        "number" => text
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .map(Value::from),
        "boolean" => match text.to_ascii_lowercase().as_str() {
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => None,
        },
        _ => None,
    }
}

/// 内核替工具写给模型的几句（`resources/core/tool-results/`）：执行之前就拦下的两句，
/// 字段是 `name`，模型说的工具名，照模板的规矩转义；打断、急着插话时补的三句，没有字段
/// （`02-内核.md` 第六节「打断和急着插话」）。
///
/// 由执行器从资源目录读好交进来，造会话时读一次，冻结在会话上。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolTexts {
    /// 工具面上没有这个名字。
    unknown: Template,
    /// 参数不是一个 JSON 对象。
    not_an_object: Template,
    /// 已取消，没跑过。
    cancelled_before: Template,
    /// 已取消，跑到一半。
    cancelled_running: Template,
    /// 已跳过。
    skipped: Template,
}

/// 那几句的原文，各是一份模板。
#[derive(Debug, Clone, Copy)]
pub struct ToolTextSources<'a> {
    /// 工具面上没有这个名字，字段 `name`。
    pub unknown: &'a str,
    /// 参数不是一个 JSON 对象，字段 `name`。
    pub not_an_object: &'a str,
    /// 已取消，没跑过。
    pub cancelled_before: &'a str,
    /// 已取消，跑到一半。
    pub cancelled_running: &'a str,
    /// 已跳过。
    pub skipped: &'a str,
}

impl ToolTexts {
    /// 读几份模板，读好以后拿字段试着换一次。
    ///
    /// # Errors
    ///
    /// 模板的写法坏了，或者要了不该有的字段，返回 [`TemplateError`]。
    pub fn new(sources: ToolTextSources<'_>) -> Result<ToolTexts, TemplateError> {
        let texts = ToolTexts {
            unknown: Template::parse(sources.unknown)?,
            not_an_object: Template::parse(sources.not_an_object)?,
            cancelled_before: Template::parse(sources.cancelled_before)?,
            cancelled_running: Template::parse(sources.cancelled_running)?,
            skipped: Template::parse(sources.skipped)?,
        };
        texts.unknown.render(&fields(""))?;
        texts.not_an_object.render(&fields(""))?;
        for plain in [
            &texts.cancelled_before,
            &texts.cancelled_running,
            &texts.skipped,
        ] {
            plain.render(&BTreeMap::new())?;
        }
        Ok(texts)
    }

    /// 工具面上没有叫 `name` 的工具。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn unknown(&self, name: &str) -> String {
        render(&self.unknown, &fields(name))
    }

    /// 给 `name` 的参数不是一个 JSON 对象。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn not_an_object(&self, name: &str) -> String {
        render(&self.not_an_object, &fields(name))
    }

    /// 已取消，没跑过。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn cancelled_before(&self) -> String {
        render(&self.cancelled_before, &BTreeMap::new())
    }

    /// 已取消，跑到一半。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn cancelled_running(&self) -> String {
        render(&self.cancelled_running, &BTreeMap::new())
    }

    /// 已跳过。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经试换过。
    pub fn skipped(&self) -> String {
        render(&self.skipped, &BTreeMap::new())
    }
}

fn render(template: &Template, fields: &BTreeMap<&str, &str>) -> String {
    template.render(fields).expect("造的时候试换过，字段都有")
}

fn fields(name: &str) -> BTreeMap<&str, &str> {
    BTreeMap::from([("name", name)])
}

#[cfg(test)]
mod tests;

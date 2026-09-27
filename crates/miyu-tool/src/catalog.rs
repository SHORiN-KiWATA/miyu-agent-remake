//! 工具目录（05 第八节，施工 4-1）：核心起来时登记一次，登记完就冻结。照名字排好，造会话时照这个
//! 先后交出工具面。

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::{Spec, Tool};

/// 工具名最长多少个字符：各家供应商对函数名的限制（05 第六节）。
const NAME_LIMIT: usize = 64;

/// 工具目录。
#[derive(Clone, Default)]
pub struct Catalog {
    tools: BTreeMap<String, Arc<dyn Tool>>,
}

/// 一件工具登记不上：是哪一件，哪一条没过。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogError {
    /// 那件工具的名字。
    pub tool: String,
    /// 哪一条没过。
    pub problem: Problem,
}

/// 登记时查的几条（05 第六节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    /// 已经有一件同名的：她调的是哪一件，说不清。
    Duplicate,
    /// 名字不合写法：只用英文字母、数字、`_`、`-`，1 到 64 个字符。不合的，每次请求都会被供应商拒收。
    Name,
    /// 参数格式不是 `{"type":"object",…}`：供应商只收对象。
    Parameters,
}

impl Catalog {
    /// 登记这几件，照名字排好。有一件查不过，整个目录都登记不上，报交进来时排在前面的那一件。
    ///
    /// # Errors
    ///
    /// 有两件同名的、名字不合写法的、参数格式不是对象的。
    pub fn new(tools: impl IntoIterator<Item = Arc<dyn Tool>>) -> Result<Catalog, CatalogError> {
        let mut catalog = Catalog::default();
        for tool in tools {
            let spec = tool.spec();
            let problem = if !name_is_valid(&spec.name) {
                Some(Problem::Name)
            } else if !takes_an_object(spec) {
                Some(Problem::Parameters)
            } else if catalog.tools.contains_key(&spec.name) {
                Some(Problem::Duplicate)
            } else {
                None
            };
            if let Some(problem) = problem {
                return Err(CatalogError {
                    tool: spec.name.clone(),
                    problem,
                });
            }
            catalog.tools.insert(spec.name.clone(), tool);
        }
        Ok(catalog)
    }

    /// 每件的规格，照名字的先后。
    pub fn specs(&self) -> impl Iterator<Item = &Spec> {
        self.tools.values().map(|tool| tool.spec())
    }
}

impl fmt::Debug for Catalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.tools.keys()).finish()
    }
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let problem = match self.problem {
            Problem::Duplicate => "another tool has the same name",
            Problem::Name => "the name must be 1 to 64 ASCII letters, digits, '_' or '-'",
            Problem::Parameters => "the parameters must be a JSON Schema of type \"object\"",
        };
        write!(f, "tool {:?}: {problem}", self.tool)
    }
}

impl std::error::Error for CatalogError {}

/// 名字合不合写法：英文字母、数字、`_`、`-`，1 到 64 个字符。
fn name_is_valid(name: &str) -> bool {
    (1..=NAME_LIMIT).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

/// 参数格式是不是 `{"type":"object",…}`。
fn takes_an_object(spec: &Spec) -> bool {
    serde_json::from_str::<Value>(spec.parameters.get())
        .ok()
        .as_ref()
        .and_then(|schema| schema.get("type"))
        .and_then(Value::as_str)
        == Some("object")
}

#[cfg(test)]
mod tests;

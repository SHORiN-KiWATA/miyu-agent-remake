//! 快照里的工具面（施工 4-1）：照名字排好，每件带访问类别。造策略时拆成两份：组装器的工具面（进 tools
//! 数组的名字、说明、参数格式），内核的工具规则（查调用、修正参数用的访问类别和参数格式）。执行器替工具
//! 写的两句也从快照里拿（施工 4-2）。一个参数照会话开局时的配置填上能选的几个，在 [`choice`]（施工 8-8 补：`subagent` 的
//! `pool`）。

use std::collections::BTreeMap;

use miyu_kernel::event::Said;
use miyu_kernel::raw::RawJson;
use miyu_kernel::request::ToolSpec;
use miyu_kernel::template::{Template, TemplateError};
use miyu_kernel::tool::{Access, ToolRule, Worded};
use serde::{Deserialize, Serialize};

use crate::snapshot::{BuildError, Snapshot};

pub(crate) mod choice;

pub use choice::Choice;

/// 工具面上的一件：名字、说明、参数格式、访问类别，照这个先后写进快照。说明和参数格式原样进 tools
/// 数组，一个字节不改。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolEntry {
    /// 工具名。
    pub name: String,
    /// 给模型看的说明。
    pub description: String,
    /// 参数的 JSON Schema，原样的 JSON。
    pub parameters: RawJson,
    /// 访问类别。
    pub access: Access,
}

/// 执行器替工具写给模型的几句（施工 4-2）：快照里有、核心的目录里没有的工具；随包卸掉了的工具（施工 F-5 中）；工具执行时
/// 崩了。和内核替工具写的那几句放在一处（`resources/core/tool-results/`），从快照里拿，和驱动的占位一样。
#[derive(Debug, Clone)]
pub struct RunTexts {
    unavailable: Template,
    crashed: Template,
    uninstalled: Template,
}

impl RunTexts {
    /// 叫 `name` 的工具现在用不了。说法是 `core/tool-results/unavailable`（施工 4-5 上）。
    pub fn unavailable(&self, name: &str) -> Worded {
        Worded {
            text: render(&self.unavailable, name),
            said: Some(Said::new("core/tool-results/unavailable").with("name", name)),
        }
    }

    /// 叫 `name` 的工具随包卸掉了（施工 F-5 中）。说法是 `core/tool-results/uninstalled`。
    pub fn uninstalled(&self, name: &str) -> Worded {
        Worded {
            text: render(&self.uninstalled, name),
            said: Some(Said::new("core/tool-results/uninstalled").with("name", name)),
        }
    }

    /// 叫 `name` 的工具崩了。说法是 `core/tool-results/crashed`。
    pub fn crashed(&self, name: &str) -> Worded {
        Worded {
            text: render(&self.crashed, name),
            said: Some(Said::new("core/tool-results/crashed").with("name", name)),
        }
    }
}

impl Snapshot {
    /// 执行器替工具写的几句。
    ///
    /// # Errors
    ///
    /// 模板的写法坏了，或者要了 `name` 以外的字段。
    pub fn run_texts(&self) -> Result<RunTexts, BuildError> {
        let results = &self.core.tool_results;
        let texts = || -> Result<RunTexts, TemplateError> {
            Ok(RunTexts {
                unavailable: parse(&results.unavailable)?,
                crashed: parse(&results.crashed)?,
                // 以前造的快照没有这一句：照「现在用不了」说。
                uninstalled: parse(match results.uninstalled.is_empty() {
                    true => &results.unavailable,
                    false => &results.uninstalled,
                })?,
            })
        };
        texts().map_err(|error| BuildError::Texts {
            which: "executor's tool result texts",
            error,
        })
    }

    /// 带上工具面：照名字排好，交进来的先后不影响字节。
    #[must_use]
    pub fn with_tools(mut self, mut tools: Vec<ToolEntry>) -> Snapshot {
        tools.sort_by(|one, other| one.name.cmp(&other.name));
        self.tools = tools;
        self
    }
}

/// 拆成组装器的工具面、内核的工具规则，照快照里的先后。
///
/// # Errors
///
/// 有两件同名的：写进 tools 数组，供应商会拒收；内核也分不清她调的是哪一件。
pub(crate) fn split(
    tools: &[ToolEntry],
) -> Result<(Vec<ToolSpec>, BTreeMap<String, ToolRule>), BuildError> {
    let mut face = Vec::with_capacity(tools.len());
    let mut rules = BTreeMap::new();
    for tool in tools {
        let rule = ToolRule {
            access: tool.access.clone(),
            parameters: tool.parameters.clone(),
        };
        if rules.insert(tool.name.clone(), rule).is_some() {
            return Err(BuildError::DuplicateTool(tool.name.clone()));
        }
        face.push(ToolSpec {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters: tool.parameters.clone(),
        });
    }
    Ok((face, rules))
}

/// 读一份带 `name` 字段的模板，拿空的名字试换一次。
fn parse(source: &str) -> Result<Template, TemplateError> {
    let template = Template::parse(source)?;
    template.render(&BTreeMap::from([("name", "")]))?;
    Ok(template)
}

/// 换进工具名。
///
/// # Panics
///
/// 实际不会 panic：造的时候已经试换过。
fn render(template: &Template, name: &str) -> String {
    template
        .render(&BTreeMap::from([("name", name)]))
        .expect("造的时候试换过，字段都有")
}

#[cfg(test)]
mod tests;

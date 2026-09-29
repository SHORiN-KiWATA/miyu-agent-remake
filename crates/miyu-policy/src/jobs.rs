//! 两种回报的写法装进快照（施工 7-2，`docs/blueprint/kernel/request.md`「回报」）：`resources/core/jobs/` 下的十一份原文。
//! 以前造的快照里没有，读成没有：那些会话派不出任务，也就没有回报。

use std::collections::BTreeMap;

use miyu_assemble::JobTexts as Rendered;
use miyu_kernel::template::Template;
use serde::{Deserialize, Serialize};

use crate::snapshot::BuildError;

/// 两种回报的写法（`jobs/` 下，文件名是下划线换成 `-` 的同名 `.txt`）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobTexts {
    /// 后台命令结束的标签：`job`、`title`、`reason`。
    pub command_open: String,
    /// 退出码：`code`。
    pub command_exit: String,
    /// 被信号杀掉：`signal`。
    pub command_signal: String,
    /// 用时：`ms`。
    pub command_duration: String,
    /// 输出有多少字、怎么看：`chars`。
    pub command_output: String,
    /// 收尾。
    pub command_close: String,
    /// 子代理的回报的标签：`job`、`title`、`reason`。
    pub subagent_open: String,
    /// 人插过话。
    pub subagent_person: String,
    /// 正文截过。
    pub subagent_truncated: String,
    /// 一个字都没说。
    pub subagent_silent: String,
    /// 收尾。
    pub subagent_close: String,
}

impl JobTexts {
    /// 交给组装器的样子：带字段的几份读成模板，拿各自的字段试换一次。
    ///
    /// # Errors
    ///
    /// 哪一份模板坏了，或者要了别的字段。
    pub(crate) fn rendered(&self) -> Result<Rendered, BuildError> {
        let tag = ["job", "title", "reason"];
        Ok(Rendered {
            command_open: template(&self.command_open, &tag)?,
            command_exit: template(&self.command_exit, &["code"])?,
            command_signal: template(&self.command_signal, &["signal"])?,
            command_duration: template(&self.command_duration, &["ms"])?,
            command_output: template(&self.command_output, &["chars"])?,
            command_close: self.command_close.clone(),
            subagent_open: template(&self.subagent_open, &tag)?,
            subagent_person: self.subagent_person.clone(),
            subagent_truncated: self.subagent_truncated.clone(),
            subagent_silent: self.subagent_silent.clone(),
            subagent_close: self.subagent_close.clone(),
        })
    }
}

/// 读一份模板，拿 `fields` 里的每个字段试换一次。
fn template(source: &str, fields: &[&str]) -> Result<Template, BuildError> {
    let bad = |error| BuildError::Texts {
        which: "job report texts",
        error,
    };
    let template = Template::parse(source).map_err(bad)?;
    let trial: BTreeMap<&str, &str> = fields.iter().map(|field| (*field, "")).collect();
    template.render(&trial).map_err(bad)?;
    Ok(template)
}

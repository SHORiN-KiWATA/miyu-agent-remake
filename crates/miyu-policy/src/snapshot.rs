//! 快照的类型，和它的字节（`docs/designs/03-事件模型.md` E5「写法」）：一个紧凑的 JSON，字段的先后
//! 就是结构体里的先后，同样的内容字节一定一样。装的是发请求要用的全部：人格、拼好的 system、随核心
//! 附带的字、几样开关。模型和供应商不在里面：同一份快照可以交给不同的端点，发请求时才定。

use std::fmt;

use miyu_assemble::{DefaultAssembler, Stable, Texts};
use miyu_drivers::{DriverTextSources, DriverTexts};
use miyu_kernel::event::{Permission, SessionCreated};
use miyu_kernel::facts::FactTemplates;
use miyu_kernel::id::{AccountId, ContentHash, VenueId};
use miyu_kernel::session::Policy;
use miyu_kernel::template::TemplateError;
use miyu_kernel::tool::{ToolTextSources, ToolTexts};
use serde::{Deserialize, Serialize};

use crate::tools::{self, ToolEntry};

/// 一份策略快照。字段的先后就是字节里的先后：改了先后，快照的字节就变了。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// 人格的编号，就是资源目录里 `personas/` 下那一层目录的名字。
    pub persona: String,
    /// 拼好的 system（`26-提示词.md` 第四节）。
    pub system: String,
    /// 工具面（施工 4-1）：照名字排好，每件带访问类别。一件都没有的不写：没有工具的快照，字节和以前
    /// 一样，M3 造的会话照旧读得回来、哈希不变。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolEntry>,
    /// 随核心附带的字。
    pub core: CoreTexts,
    /// 一个回合最多请求几次模型；没有就是不限。初值等 M4 有了工具再定（施工 2-4 留下的）。
    pub step_limit: Option<u32>,
    /// 有没有人能确认：终端有；不是终端时的 `miyu ask` 没有（`02-内核.md` 第六节「确认怎么走」）。
    pub attended: bool,
    /// 有计划的重启打断了一轮，再起来时连着接着干几次（`02-内核.md` 第六节「载入、崩溃、重启」）。
    pub resumes: u32,
}

/// 随核心附带的字（`resources/core/`），原文照抄，行尾的换行也算（`26-提示词.md` 第八节）。
/// 装进快照：核心升级改了这些字，老会话照样逐字节重现当时的请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreTexts {
    /// 压缩检查点的开头（`checkpoint-open.txt`）。
    pub checkpoint_open: String,
    /// 压缩检查点的结尾（`checkpoint-close.txt`）。
    pub checkpoint_close: String,
    /// 回合没走完的几句（`turn-ended/`）。
    pub turn_ended: TurnEndedTexts,
    /// 事实的模板（`facts/`）。
    pub facts: FactTexts,
    /// 内核替工具写给模型的几句（`tool-results/`）。
    pub tool_results: ToolResultTexts,
    /// 驱动的几句占位（`drivers/`）。
    pub drivers: DriverPlaceholders,
    /// 权限策略拒绝时写给她的两句（`permissions/`，施工 4-3 下）。这一格以前造的会话里没有，读成空的：那时
    /// 的会话没有工具，用不到它们。
    #[serde(default)]
    pub permissions: PermissionTexts,
}

/// 权限策略拒绝时写给她的两句（施工 4-3 下）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionTexts {
    /// 碰到了数据根（`forbidden.txt`）。
    pub forbidden: String,
    /// 路径换不成真实的位置（`unresolvable.txt`）。
    pub unresolvable: String,
}

/// 回合没走完的几句，照原因。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnEndedTexts {
    /// 被人打断（`interrupted.txt`）。
    pub interrupted: String,
    /// 出错（`error.txt`）。
    pub error: String,
    /// 走到了步数上限（`step_limit.txt`）。
    pub step_limit: String,
    /// 核心崩了，没走完（`aborted.txt`）。
    pub aborted: String,
    /// 被有计划的重启打断（`restarted.txt`）。
    pub restarted: String,
}

/// 事实的模板。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactTexts {
    /// 环境（`env.txt`）。
    pub env: String,
    /// 权限级别（`permission.txt`）。
    pub permission: String,
    /// 回复没说完就断了（`reply-cut.txt`）。
    pub reply_cut: String,
}

/// 内核替工具写给模型的几句，名字照 `resources/core/tool-results/` 里的文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResultTexts {
    /// 没有这件工具（`unknown.txt`）。
    pub unknown: String,
    /// 参数不是 JSON 对象（`not-an-object.txt`）。
    pub not_an_object: String,
    /// 还没开始就被打断（`cancelled-before.txt`）。
    pub cancelled_before: String,
    /// 跑到一半被打断（`cancelled-running.txt`）。
    pub cancelled_running: String,
    /// 急着插话，这一步没跑的（`skipped.txt`）。
    pub skipped: String,
    /// 只读时拦下写入的（`read-only.txt`）。
    pub read_only: String,
    /// 人拒绝了（`denied.txt`）。
    pub denied: String,
    /// 人拒绝了，还说了理由（`denied-with-reason.txt`）。
    pub denied_with_reason: String,
    /// 要确认却没人能确认（`unattended.txt`）。
    pub unattended: String,
    /// 问人的时候被打断（`question-interrupted.txt`）。
    pub question_interrupted: String,
    /// 问的题作废了（`question-voided.txt`）。
    pub question_voided: String,
    /// 要问人却没人能回答（`question-unattended.txt`）。
    pub question_unattended: String,
    /// 有计划的重启打断了调用（`restarted.txt`）。
    pub restarted: String,
    /// 快照里有、核心的目录里没有的工具（`unavailable.txt`，施工 4-2）：执行器写。这一格以前造的会话
    /// 里没有，读成空的：那时的会话没有工具，用不到它。
    #[serde(default)]
    pub unavailable: String,
    /// 工具执行时崩了（`crashed.txt`，施工 4-2）：执行器写。读不到的同上。
    #[serde(default)]
    pub crashed: String,
}

/// 驱动的几句占位。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverPlaceholders {
    /// 图片发不了（`image-omitted.txt`）。
    pub image_omitted: String,
    /// 文件读不了（`file-omitted.txt`）。
    pub file_omitted: String,
    /// 工具一个字都没回（`no-output.txt`）。
    pub no_output: String,
    /// 工具结果里的附件挪到了后面（`tool-attachments.txt`）。
    pub tool_attachments: String,
    /// 工具结果只有附件，挪到了后面（`tool-attachments-only.txt`）。
    pub tool_attachments_only: String,
}

/// 快照的字节读不回来：不是这个版本写的，或者坏了。还没发布，格式改了不背兼容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotError(String);

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "policy snapshot not readable: {}", self.0)
    }
}

impl std::error::Error for SnapshotError {}

/// 照快照造不出策略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// 随核心附带的哪一份字用不了。
    Texts {
        /// 哪一类：事实的模板、内核替工具写的几句、驱动的占位。
        which: &'static str,
        /// 哪里坏了。
        error: TemplateError,
    },
    /// 工具面上有两件叫这个名字的（施工 4-1）：她调的是哪一件，说不清。
    DuplicateTool(String),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::Texts { which, error } => write!(f, "bundled {which} not usable: {error}"),
            BuildError::DuplicateTool(name) => write!(f, "two tools named {name:?}"),
        }
    }
}

impl std::error::Error for BuildError {}

impl Snapshot {
    /// 规范的字节：紧凑的 JSON，字段照结构体的先后。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：快照里只有字符串、数字和开关，写成 JSON 不会失败。
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("快照里只有字符串、数字和开关，写成 JSON 不会失败")
    }

    /// 内容哈希：规范字节的 SHA-256。存成 blob 用的、`session.created` 记的，都是它。
    pub fn hash(&self) -> ContentHash {
        ContentHash::of(&self.to_bytes())
    }

    /// 从字节读回来。
    ///
    /// # Errors
    ///
    /// 不是这个版本写的快照，或者字节坏了。
    pub fn from_bytes(bytes: &[u8]) -> Result<Snapshot, SnapshotError> {
        serde_json::from_slice(bytes).map_err(|error| SnapshotError(error.to_string()))
    }

    /// 造会话的那一条：属主、场所、这份快照的哈希、开始时的权限（`03-事件模型.md` 第三节）。工作目录由造会话的一方
    /// 填上。
    pub fn session_created(
        &self,
        owner: AccountId,
        venue: VenueId,
        permission: Permission,
    ) -> SessionCreated {
        SessionCreated {
            owner,
            venue,
            policy: self.hash(),
            permission,
            oneshot: false,
            cwd: None,
        }
    }

    /// 照快照造出内核的策略：组装器（稳定区有工具面和 system，示范对话随人格那一步）、事实模板、每件
    /// 工具的访问类别和参数格式、内核替工具写的几句、几样开关。
    ///
    /// # Errors
    ///
    /// 随核心附带的哪一份字用不了，写明是哪一类、哪里坏了；工具面上有两件同名的。
    pub fn policy(&self) -> Result<Policy, BuildError> {
        let core = &self.core;
        let ended = &core.turn_ended;
        let texts = Texts {
            checkpoint_open: core.checkpoint_open.clone(),
            checkpoint_close: core.checkpoint_close.clone(),
            turn_ended: miyu_assemble::TurnEndedTexts {
                interrupted: ended.interrupted.clone(),
                error: ended.error.clone(),
                step_limit: ended.step_limit.clone(),
                aborted: ended.aborted.clone(),
                restarted: ended.restarted.clone(),
            },
        };
        let (face, rules) = tools::split(&self.tools)?;
        let stable = Stable {
            tools: face,
            system: self.system.clone(),
            demos: Vec::new(),
        };
        let facts = FactTemplates::new(
            &core.facts.env,
            &core.facts.permission,
            &core.facts.reply_cut,
        )
        .map_err(|error| BuildError::Texts {
            which: "fact templates",
            error,
        })?;
        Ok(Policy {
            assembler: Box::new(DefaultAssembler::new(stable, texts)),
            facts,
            tools: rules,
            step_limit: self.step_limit,
            tool_texts: self.tool_texts()?,
            attended: self.attended,
            resumes: self.resumes,
        })
    }

    /// 驱动的占位：图片、文件发不了，工具一个字都没回，附件挪到了后面。
    ///
    /// # Errors
    ///
    /// 占位的模板坏了。
    pub fn driver_texts(&self) -> Result<DriverTexts, BuildError> {
        let drivers = &self.core.drivers;
        DriverTexts::new(DriverTextSources {
            image_omitted: &drivers.image_omitted,
            file_omitted: &drivers.file_omitted,
            no_output: &drivers.no_output,
            tool_attachments: &drivers.tool_attachments,
            tool_attachments_only: &drivers.tool_attachments_only,
        })
        .map_err(|error| BuildError::Texts {
            which: "driver placeholders",
            error,
        })
    }

    /// 内核替工具写的几句。
    fn tool_texts(&self) -> Result<ToolTexts, BuildError> {
        let results = &self.core.tool_results;
        ToolTexts::new(ToolTextSources {
            unknown: &results.unknown,
            not_an_object: &results.not_an_object,
            cancelled_before: &results.cancelled_before,
            cancelled_running: &results.cancelled_running,
            skipped: &results.skipped,
            read_only: &results.read_only,
            denied: &results.denied,
            denied_with_reason: &results.denied_with_reason,
            unattended: &results.unattended,
            question_interrupted: &results.question_interrupted,
            question_voided: &results.question_voided,
            question_unattended: &results.question_unattended,
            restarted: &results.restarted,
        })
        .map_err(|error| BuildError::Texts {
            which: "kernel's tool result texts",
            error,
        })
    }
}

#[cfg(test)]
mod tests;

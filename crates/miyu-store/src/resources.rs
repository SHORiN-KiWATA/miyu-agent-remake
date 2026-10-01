//! 资源目录（`docs/designs/12-进程形态与分发.md` 第三节「资源目录的位置」「怎么找」，施工 3-6 上）：
//! 出厂的人格、提示词都在这里，随安装包一起分发，不编进二进制（R3）。
//!
//! 怎么找：先看环境变量 `MIYU_RESOURCES`，开发时把它指到源码树的 `resources/`；没设的，看程序的真实
//! 位置，旁边有 `resources/` 就是它（安装脚本），上一级有 `share/miyu/` 就是它（deb、rpm、AUR、
//! Homebrew）。都没有，说清找过哪几处，不猜别的位置。
//!
//! 读出来的原文交给第 2 层去拼快照（`miyu-policy`）。

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use miyu_policy::{
    CompactionTexts, CoreLines, CoreTexts, DriverPlaceholders, FactTexts, HarnessTexts,
    ImageNameTexts, JobTexts, PeerTexts, PermissionTexts, PersonaTexts, RebuildTexts, RecapTexts,
    ShortenTexts, Sources, TextFileTexts, TitleTexts, ToolResultTexts, TurnEndedTexts,
};

use crate::env::Env;

/// 一个资源目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceRoot {
    path: PathBuf,
}

/// 找不到资源目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceError {
    /// `MIYU_RESOURCES` 是相对路径：进程换个工作目录，指的就是别处了。
    Relative(PathBuf),
    /// `MIYU_RESOURCES` 指的地方不是一个目录。
    Missing(PathBuf),
    /// 没设 `MIYU_RESOURCES`，程序旁边、上一级都没有。里面是找过的几处；程序的位置都不知道的，是空的。
    NotFound(Vec<PathBuf>),
}

impl fmt::Display for ResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResourceError::Relative(path) => {
                write!(f, "MIYU_RESOURCES 要写绝对路径，现在是 {}", path.display())
            }
            ResourceError::Missing(path) => {
                write!(f, "MIYU_RESOURCES 指的 {} 不是一个目录", path.display())
            }
            ResourceError::NotFound(tried) if tried.is_empty() => write!(
                f,
                "找不到资源目录：不知道程序在哪。开发时设 MIYU_RESOURCES 指到源码树的 resources/"
            ),
            ResourceError::NotFound(tried) => {
                let tried: Vec<String> = tried.iter().map(|p| p.display().to_string()).collect();
                write!(
                    f,
                    "找不到资源目录：{} 都没有。开发时设 MIYU_RESOURCES 指到源码树的 resources/",
                    tried.join("、")
                )
            }
        }
    }
}

impl std::error::Error for ResourceError {}

/// 读不出一个人格要用的原文。
#[derive(Debug)]
pub enum SourceError {
    /// 人格的编号不合写法：小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符。
    Persona(String),
    /// 读不了这一份文件。
    Read {
        /// 哪一份。
        path: PathBuf,
        /// 为什么读不了。
        error: io::Error,
    },
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceError::Persona(persona) => write!(
                f,
                "persona id {persona:?} is not valid: it starts with a lowercase letter and has only lowercase letters, digits, - and _"
            ),
            SourceError::Read { path, error } => {
                write!(f, "cannot read {}: {error}", path.display())
            }
        }
    }
}

impl std::error::Error for SourceError {}

impl ResourceRoot {
    /// 照环境快照找资源目录。
    ///
    /// # Errors
    ///
    /// `MIYU_RESOURCES` 是相对路径，或者指的不是目录；没设的时候，程序旁边的 `resources/`、上一级的
    /// `share/miyu/` 都没有。
    pub fn locate(env: &Env) -> Result<ResourceRoot, ResourceError> {
        if let Some(dir) = env.miyu_resources.as_ref().filter(|dir| !dir.is_empty()) {
            // 开头的 `~` 照家目录接（施工 4-11）；家目录找不到的，照原样，下面当相对路径报错。
            let path = env.expand(dir).unwrap_or_else(|| PathBuf::from(dir));
            if !path.is_absolute() {
                return Err(ResourceError::Relative(path));
            }
            return match path.is_dir() {
                true => Ok(ResourceRoot { path }),
                false => Err(ResourceError::Missing(path)),
            };
        }
        let Some(dir) = env.exe.as_deref().and_then(Path::parent) else {
            return Err(ResourceError::NotFound(Vec::new()));
        };
        let mut tried = vec![dir.join("resources")];
        if let Some(prefix) = dir.parent() {
            tried.push(prefix.join("share").join("miyu"));
        }
        match tried.iter().find(|candidate| candidate.is_dir()) {
            Some(found) => Ok(ResourceRoot {
                path: found.clone(),
            }),
            None => Err(ResourceError::NotFound(tried)),
        }
    }

    /// 就用 `path` 这个目录：测试、工具指定的。
    pub fn at(path: impl Into<PathBuf>) -> ResourceRoot {
        ResourceRoot { path: path.into() }
    }

    /// 资源目录本身。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 读出人格 `persona` 要用的原文：随核心附带的字（`core/`），这个人格的人设
    /// （`personas/<编号>/prompts/persona.md`）。原文照抄，行尾的换行也算。
    ///
    /// # Errors
    ///
    /// 人格的编号不合写法；哪一份文件读不了，写明是哪一份。
    pub fn sources(&self, persona: &str) -> Result<Sources, SourceError> {
        if !valid(persona) {
            return Err(SourceError::Persona(persona.to_string()));
        }
        Ok(Sources {
            core: self.core()?,
            persona: PersonaTexts {
                persona: self.read(&["personas", persona, "prompts", "persona.md"])?,
            },
        })
    }

    /// 子会话的场所说明（施工 7-5，`agents.md` 第九条第 3 条）：`core/jobs/subagent-venue.txt` 的原文，造子会话时接在人设
    /// 后面（`Snapshot::with_venue`）。只在造子会话时读：别的会话的快照里没有它。
    ///
    /// # Errors
    ///
    /// 读不了这份文件，写明是哪一份。
    pub fn subagent_venue(&self) -> Result<String, SourceError> {
        self.read(&["core", "jobs", "subagent-venue.txt"])
    }

    /// 核心的几行（施工 2-7 补，`26-提示词.md` 第四节第 3 块）：`core/permission-rule.txt`、`core/local-paths-rule.txt` 的
    /// 原文，造会话时拼进 system（`Snapshot::with_core_lines`）。只在造会话时读：以前造的快照 system 里没有它们。
    ///
    /// # Errors
    ///
    /// 读不了其中一份，写明是哪一份。
    pub fn core_lines(&self) -> Result<CoreLines, SourceError> {
        Ok(CoreLines {
            permission: self.read(&["core", "permission-rule.txt"])?,
            local_paths: self.read(&["core", "local-paths-rule.txt"])?,
        })
    }

    /// 随核心附带的字。
    fn core(&self) -> Result<CoreTexts, SourceError> {
        let core = |parts: &[&str]| {
            let mut path = vec!["core"];
            path.extend_from_slice(parts);
            self.read(&path)
        };
        let ended = |name: &str| core(&["turn-ended", name]);
        let fact = |name: &str| core(&["facts", name]);
        let result = |name: &str| core(&["tool-results", name]);
        let driver = |name: &str| core(&["drivers", name]);
        let job = |name: &str| core(&["jobs", name]);
        Ok(CoreTexts {
            checkpoint_open: core(&["checkpoint-open.txt"])?,
            checkpoint_close: core(&["checkpoint-close.txt"])?,
            checkpoint_end: core(&["checkpoint-end.txt"])?,
            turn_ended: TurnEndedTexts {
                interrupted: ended("interrupted.txt")?,
                error: ended("error.txt")?,
                step_limit: ended("step_limit.txt")?,
                aborted: ended("aborted.txt")?,
                restarted: ended("restarted.txt")?,
            },
            facts: FactTexts {
                env: fact("env.txt")?,
                permission: fact("permission.txt")?,
                reply_cut: fact("reply-cut.txt")?,
                session: Some(fact("session.txt")?),
                permission_changed: Some(fact("permission-changed.txt")?),
            },
            tool_results: ToolResultTexts {
                unknown: result("unknown.txt")?,
                not_an_object: result("not-an-object.txt")?,
                cancelled_before: result("cancelled-before.txt")?,
                cancelled_running: result("cancelled-running.txt")?,
                skipped: result("skipped.txt")?,
                read_only: result("read-only.txt")?,
                denied: result("denied.txt")?,
                denied_with_reason: result("denied-with-reason.txt")?,
                unattended: result("unattended.txt")?,
                question_interrupted: result("question-interrupted.txt")?,
                question_voided: result("question-voided.txt")?,
                question_unattended: result("question-unattended.txt")?,
                restarted: result("restarted.txt")?,
                unavailable: result("unavailable.txt")?,
                crashed: result("crashed.txt")?,
            },
            permissions: PermissionTexts {
                forbidden: core(&["permissions", "forbidden.txt"])?,
                unresolvable: core(&["permissions", "unresolvable.txt"])?,
            },
            drivers: DriverPlaceholders {
                image_omitted: driver("image-omitted.txt")?,
                file_omitted: driver("file-omitted.txt")?,
                no_output: driver("no-output.txt")?,
                tool_attachments: driver("tool-attachments.txt")?,
                tool_attachments_only: driver("tool-attachments-only.txt")?,
                text_file: Some(TextFileTexts {
                    file_open: driver("file-open.txt")?,
                    file_cut: driver("file-cut.txt")?,
                    file_close: driver("file-close.txt")?,
                }),
                image_name: Some(ImageNameTexts {
                    image_open: driver("image-open.txt")?,
                    image_close: driver("image-close.txt")?,
                    image_omitted_named: driver("image-omitted-named.txt")?,
                }),
            },
            compaction: Some(CompactionTexts {
                summarize_task: core(&["compaction", "summarize-task.txt"])?,
                summarize_instructions: core(&["compaction", "summarize-instructions.txt"])?,
                summarize_end: core(&["compaction", "summarize-end.txt"])?,
                rebuild: Some(RebuildTexts {
                    notes_files: core(&["compaction", "notes-files.txt"])?,
                    notes_files_more: core(&["compaction", "notes-files-more.txt"])?,
                    notes_retrieve: core(&["compaction", "notes-retrieve.txt"])?,
                    notes_too_large: core(&["compaction", "notes-too-large.txt"])?,
                    restored_open: core(&["compaction", "restored-open.txt"])?,
                    restored_close: core(&["compaction", "restored-close.txt"])?,
                }),
                summarize_system: Some(core(&["compaction", "summarize-system.txt"])?),
                shorten: Some(ShortenTexts {
                    truncated: core(&["compaction", "truncated.txt"])?,
                    notes_uncovered: core(&["compaction", "notes-uncovered.txt"])?,
                }),
            }),
            jobs: Some(JobTexts {
                command_open: job("command-open.txt")?,
                command_exit: job("command-exit.txt")?,
                command_signal: job("command-signal.txt")?,
                command_duration: job("command-duration.txt")?,
                command_output: job("command-output.txt")?,
                command_close: job("command-close.txt")?,
                subagent_open: job("subagent-open.txt")?,
                subagent_person: job("subagent-person.txt")?,
                subagent_truncated: job("subagent-truncated.txt")?,
                subagent_silent: job("subagent-silent.txt")?,
                subagent_close: job("subagent-close.txt")?,
                subagent_omitted: job("subagent-omitted.txt")?,
                stopped_by_user: job("stopped-by-user.txt")?,
                subagent_message_open: job("subagent-message-open.txt")?,
                subagent_message_close: job("subagent-message-close.txt")?,
            }),
            harness: Some(HarnessTexts {
                message_open: core(&["harness", "message-open.txt"])?,
                message_close: core(&["harness", "message-close.txt"])?,
            }),
            peers: Some(PeerTexts {
                message_open: core(&["peers", "message-open.txt"])?,
                message_close: core(&["peers", "message-close.txt"])?,
            }),
            recap: Some(RecapTexts {
                instruction: core(&["recap", "instruction.txt"])?,
                user: core(&["recap", "user.txt"])?,
                assistant: core(&["recap", "assistant.txt"])?,
                omitted: core(&["recap", "omitted.txt"])?,
                excerpted: core(&["recap", "excerpted.txt"])?,
            }),
            title: Some(TitleTexts {
                instruction: core(&["title", "instruction.txt"])?,
            }),
        })
    }

    /// models.dev 目录的快照在哪（`models/models-dev.json`，施工 8-7）：原样的 `api.json`，旁边的 `models-dev.meta.json` 是
    /// 它是什么时候拉的。约 5 MB，由核心起来以后在后台读；读不了的照样起来（`models.md`「怎么走」第二条第 1 条）。
    pub fn catalog_snapshot(&self) -> std::path::PathBuf {
        self.path.join("models").join("models-dev.json")
    }

    /// 认原厂的表的原文（`models/vendors.toml`，施工 8-7）。怎么读由核心定。
    ///
    /// # Errors
    ///
    /// 读不出来：写明是哪个文件。
    pub fn vendors(&self) -> Result<String, SourceError> {
        self.read(&["models", "vendors.toml"])
    }

    /// 供应商的档案的原文（`models/profiles.toml`，施工 8-6）：认得出的供应商的驱动、地址、开关。怎么读由核心定。
    ///
    /// # Errors
    ///
    /// 读不出来：写明是哪个文件。
    pub fn profiles(&self) -> Result<String, SourceError> {
        self.read(&["models", "profiles.toml"])
    }

    /// 读资源目录下的一份文件，路径一段一段地接上（三个平台一样）。
    fn read(&self, parts: &[&str]) -> Result<String, SourceError> {
        let path = parts
            .iter()
            .fold(self.path.clone(), |path, part| path.join(part));
        std::fs::read_to_string(&path).map_err(|error| SourceError::Read { path, error })
    }
}

/// 人格的编号合不合写法：它是资源目录里的一层目录，不许带路径。
fn valid(persona: &str) -> bool {
    let mut chars = persona.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && persona.len() <= 64
}

#[cfg(test)]
mod tests;

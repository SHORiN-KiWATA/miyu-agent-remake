//! 执行工具的端口（`02-内核.md` 第四节「执行工具」「停下工具」，`05-内核接口.md` 第六节「执行这一步」，
//! 施工 4-2）：照工具名在目录里找到那一件，一件一个任务地跑，量用时；叫停就掐掉那个任务。回报送回
//! actor 的收件箱，由它写成内核的输入。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use tokio::sync::mpsc;
use tokio::task::AbortHandle;
use tracing::Instrument;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::id::CallId;
use miyu_kernel::session::Input;
use miyu_kernel::time::Timestamp;
use miyu_policy::RunTexts;
use miyu_tool::{Call, Catalog, Done, Progress};

use crate::TARGET;
use crate::lines::millis;
use crate::port::Back;

/// 执行工具要的：工具目录、替工具写的两句、系统的家目录（施工 4-4 上，交给每次调用）。
pub(crate) struct ToolKit {
    /// 工具目录。
    pub(crate) catalog: Catalog,
    /// 替工具写的两句。
    pub(crate) texts: RunTexts,
    /// 系统的家目录。
    pub(crate) home: Option<PathBuf>,
}

/// 执行工具的端口：一个会话一份。
pub(crate) struct Tools {
    catalog: Catalog,
    texts: RunTexts,
    home: Option<PathBuf>,
    /// 在跑的调用：掐掉它的那一头、开始跑的那一刻、工具名。
    running: BTreeMap<CallId, Running>,
    backs: mpsc::UnboundedSender<Back>,
}

/// 一次在跑的调用。
struct Running {
    task: AbortHandle,
    started: Instant,
    name: String,
}

/// 跑工具的任务送回来的。
#[derive(Debug)]
pub(crate) enum ToolBack {
    /// 执行中的一段输出。
    Progress { call_id: CallId, text: String },
    /// 跑完了。
    Done { call_id: CallId, done: Done },
    /// 工具自己崩了（panic）。
    Crashed { call_id: CallId },
}

impl Tools {
    /// 照 `kit` 跑，回报送进 `backs`。
    pub(crate) fn new(kit: ToolKit, backs: mpsc::UnboundedSender<Back>) -> Tools {
        Tools {
            catalog: kit.catalog,
            texts: kit.texts,
            home: kit.home,
            running: BTreeMap::new(),
            backs,
        }
    }

    /// 执行一次调用：在自己的任务里跑，马上返回。目录里没有这件工具的，不派，当场交回出错的结果。
    pub(crate) fn run(
        &mut self,
        at: Timestamp,
        call_id: CallId,
        name: String,
        args: String,
        cwd: String,
    ) -> Option<Input> {
        let call = Call {
            args,
            cwd,
            home: self.home.clone(),
        };
        let call_text = call_id.to_string();
        let Some(tool) = self.catalog.get(&name).cloned() else {
            tracing::warn!(target: TARGET, call = call_text.as_str(), tool = name.as_str(), "unavailable");
            return Some(Input::ToolDone {
                at,
                call_id,
                error: true,
                blocks: text(self.texts.unavailable(&name)),
                duration_ms: None,
            });
        };
        tracing::info!(target: TARGET, call = call_text.as_str(), tool = name.as_str(), "running");
        let progress = {
            let backs = self.backs.clone();
            Progress::new(move |text| send(&backs, ToolBack::Progress { call_id, text }))
        };
        let span = tracing::Span::current();
        let inner = tokio::spawn(async move { tool.run(call, progress).await }.instrument(span));
        let task = inner.abort_handle();
        let backs = self.backs.clone();
        tokio::spawn(async move {
            match inner.await {
                Ok(done) => send(&backs, ToolBack::Done { call_id, done }),
                Err(error) if error.is_panic() => send(&backs, ToolBack::Crashed { call_id }),
                // 叫停了：没人要了。
                Err(_) => {}
            }
        });
        self.running.insert(
            call_id,
            Running {
                task,
                started: Instant::now(),
                name,
            },
        );
        None
    }

    /// 停下一次在跑的调用：掐掉跑它的任务。之后什么都不再报。
    pub(crate) fn cancel(&mut self, call_id: CallId) {
        let Some(running) = self.running.remove(&call_id) else {
            return;
        };
        running.task.abort();
        tracing::info!(
            target: TARGET,
            call = call_id.to_string().as_str(),
            took_ms = millis(running.started.elapsed()),
            "stopped"
        );
    }

    /// 跑工具的任务送回来的，写成内核的输入。不在跑的（已经叫停了的）不理。
    pub(crate) fn back(&mut self, at: Timestamp, back: ToolBack) -> Option<Input> {
        match back {
            ToolBack::Progress { call_id, text } => self
                .running
                .contains_key(&call_id)
                .then_some(Input::ToolProgress { at, call_id, text }),
            ToolBack::Done { call_id, done } => {
                let running = self.running.remove(&call_id)?;
                let took_ms = millis(running.started.elapsed());
                tracing::info!(
                    target: TARGET,
                    call = call_id.to_string().as_str(),
                    took_ms,
                    error = done.error.then_some(true),
                    "ran"
                );
                Some(Input::ToolDone {
                    at,
                    call_id,
                    error: done.error,
                    blocks: done.blocks,
                    duration_ms: Some(took_ms),
                })
            }
            ToolBack::Crashed { call_id } => {
                let running = self.running.remove(&call_id)?;
                let took_ms = millis(running.started.elapsed());
                tracing::error!(
                    target: TARGET,
                    call = call_id.to_string().as_str(),
                    tool = running.name.as_str(),
                    took_ms,
                    "crashed"
                );
                Some(Input::ToolDone {
                    at,
                    call_id,
                    error: true,
                    blocks: text(self.texts.crashed(&running.name)),
                    duration_ms: Some(took_ms),
                })
            }
        }
    }
}

/// 会话停了，在跑的工具都掐掉：结果没人要了。
impl Drop for Tools {
    fn drop(&mut self) {
        for running in self.running.values() {
            running.task.abort();
        }
    }
}

/// 一段字的内容块。
fn text(text: String) -> Vec<Block> {
    vec![Block::Text(Text { text })]
}

/// 送回 actor。会话停了就送不进去，丢掉。
#[expect(
    clippy::let_underscore_must_use,
    reason = "会话停了：工具的回报没人要了，丢掉"
)]
fn send(backs: &mpsc::UnboundedSender<Back>, back: ToolBack) {
    let _ = backs.send(Back::Tool(back));
}

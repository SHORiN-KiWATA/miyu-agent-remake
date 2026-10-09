//! 跑工具的任务送回来的，写成内核的输入（从 `tools.rs` 挪来，施工 D-2：那边要放提问的端口，过了 500 行）。

use std::sync::Arc;

use miyu_kernel::session::Input;
use miyu_kernel::time::Timestamp;

use super::{ToolBack, Tools, text};
use crate::TARGET;
use crate::effects;
use crate::lines::millis;

impl Tools {
    /// 跑工具的任务送回来的，写成内核的输入。不在跑的（已经叫停了的）不理。
    pub(crate) fn back(&mut self, at: Timestamp, back: ToolBack) -> Option<Input> {
        match back {
            ToolBack::Asks {
                call_id,
                questions,
                reply,
            } => self.asked(at, call_id, questions, reply),
            ToolBack::Progress { call_id, text } => self
                .running
                .contains_key(&call_id)
                .then_some(Input::ToolProgress { at, call_id, text }),
            ToolBack::Done {
                call_id,
                done,
                effects,
            } => {
                let running = self.running.remove(&call_id)?;
                effects::saw(Arc::make_mut(&mut self.seen), &effects);
                let took_ms = millis(running.started.elapsed());
                tracing::info!(
                    target: TARGET,
                    call = call_id.to_string().as_str(),
                    took_ms,
                    error = done.error.then_some(true),
                    stopped = done.stopped.then_some(true),
                    "ran"
                );
                Some(Input::ToolDone {
                    at,
                    call_id,
                    error: done.error,
                    blocks: done.blocks,
                    duration_ms: Some(took_ms),
                    human: done.human,
                    effects,
                    stopped: done.stopped,
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
                let worded = self.lettering.run().crashed(&running.name);
                Some(Input::ToolDone {
                    at,
                    call_id,
                    error: true,
                    blocks: text(worded.text),
                    duration_ms: Some(took_ms),
                    human: worded.said,
                    effects: Vec::new(),
                    stopped: false,
                })
            }
        }
    }
}

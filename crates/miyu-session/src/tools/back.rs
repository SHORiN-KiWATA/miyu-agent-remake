//! 跑工具的任务送回来的，写成内核的输入（从 `tools.rs` 挪来，施工 D-2：那边要放提问的端口，过了 500 行）。

use std::sync::Arc;

use miyu_kernel::event::Effect;
use miyu_kernel::session::Input;
use miyu_kernel::time::Timestamp;

use super::{ToolBack, Tools, text};
use crate::TARGET;
use crate::effects;
use crate::lines::millis;

impl Tools {
    /// 结果没人要了（调用已经被掐掉）里派出去的子代理（施工 7-5 补）：没记成任务，没人管，停掉它。
    fn unclaimed(&self, effects: &[Effect]) {
        let Some(agents) = self.agents() else {
            return;
        };
        for (job, child) in crate::agents::spawned_in(effects) {
            agents.stop_unclaimed(&job, child);
        }
    }

    /// 跑工具的任务送回来的，写成内核的输入。不在跑的（已经叫停了的）不理，派出去的子代理停掉（施工 7-5 补）。
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
                let Some(running) = self.running.remove(&call_id) else {
                    self.unclaimed(&effects);
                    return None;
                };
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

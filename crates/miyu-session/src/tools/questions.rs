//! 问人（施工 D-2，`docs/blueprint/session/tools.md`「问人」，`tools/ask_user.md`）：执行器这一头的提问端口。
//!
//! - 一次调用一个端口，只给能问人的会话（[`port`]，`Agents::asks`）：工具面上的 `ask_user` 也只有它们有。
//! - 工具交题 → 经回传的通道送回 actor（[`ToolBack::Asks`]）→ 记下回信的口子、交给内核 `ToolAsks`，内核记 `question.asked`、等。
//! - 人回答、`question.answered` 落了盘，内核出 `AnswerTool` → [`Tools::answer`] 照调用编号送回去。
//! - 没答到就了结的：叫停（[`Tools::stop`]）、掐掉（打断、来了一句话、收紧成只读，内核出 `CancelTool`）都把口子丢掉，工具那一头
//!   拿到 `None` 收场；它交回的结果内核不认（这次调用已经有结果了）。

use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use miyu_kernel::event::{Question, Response};
use miyu_kernel::id::CallId;
use miyu_kernel::session::Input;
use miyu_kernel::time::Timestamp;
use miyu_tool::{Answering, QuestionPort};

use super::{ToolBack, Tools, send};
use crate::port::Back;

/// 一次调用的提问端口：交来的题照调用编号送回 actor。
struct Asker {
    call_id: CallId,
    backs: mpsc::UnboundedSender<Back>,
}

impl QuestionPort for Asker {
    fn ask(&self, questions: Vec<Question>) -> Answering<'_> {
        let (reply, answered) = oneshot::channel();
        send(
            &self.backs,
            ToolBack::Asks {
                call_id: self.call_id,
                questions,
                reply,
            },
        );
        // 口子被丢掉（没答到就了结了、会话停了）：没有回答。
        Box::pin(async move { answered.await.ok() })
    }
}

/// 交给第 `call_id` 次调用的提问端口：能问人（`asks`，造会话、载入时照 `Agents::asks` 定的）才有。
pub(super) fn port(
    asks: bool,
    call_id: CallId,
    backs: &mpsc::UnboundedSender<Back>,
) -> Option<Arc<dyn QuestionPort>> {
    asks.then(|| {
        Arc::new(Asker {
            call_id,
            backs: backs.clone(),
        }) as Arc<dyn QuestionPort>
    })
}

impl Tools {
    /// 第 `call_id` 次调用交了题：记下回信的口子 `reply`，交给内核。不在跑的、叫它停过的不理：口子跟着丢，工具当没答收场。
    pub(super) fn asked(
        &mut self,
        at: Timestamp,
        call_id: CallId,
        questions: Vec<Question>,
        reply: oneshot::Sender<Vec<Response>>,
    ) -> Option<Input> {
        let running = self.running.get_mut(&call_id)?;
        if running.stopping {
            return None;
        }
        running.answer = Some(reply);
        Some(Input::ToolAsks {
            at,
            call_id,
            questions,
        })
    }

    /// 内核交来第 `call_id` 次调用的回答（已经落了盘）：送给在等的工具。不在等的不理。
    #[expect(
        clippy::let_underscore_must_use,
        reason = "工具已经不在了：回答没人要，内核那边照常了结"
    )]
    pub(crate) fn answer(&mut self, call_id: CallId, answers: Vec<Response>) {
        if let Some(reply) = self
            .running
            .get_mut(&call_id)
            .and_then(|running| running.answer.take())
        {
            let _ = reply.send(answers);
        }
    }
}

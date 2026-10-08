//! 握手以后几个方法的参数（从 `methods.rs` 挪出来，那边放不下了）。

use serde::Deserialize;

use miyu_kernel::event::{Decision, Response};
use miyu_kernel::session::Answer;

use crate::attach::Attachment;
use crate::refusal::Refusal;

/// `session.create` 的参数。
#[derive(Debug, Deserialize)]
pub(super) struct CreateParams {
    /// 用哪个人格：不写的照 `persona.default`，写 `null` 的明着无人格（施工 P-4 上）。
    #[serde(default, deserialize_with = "crate::personas::written")]
    pub(super) persona: Option<Option<String>>,
    /// 用哪个预设（施工 P-2 上）：不写、写 `null` 的照这时的 `preset.default`。
    #[serde(default)]
    pub(super) preset: Option<String>,
    pub(super) cwd: String,
    /// 一次性的：`miyu ask` 开的（施工 3-9 下）。
    #[serde(default)]
    pub(super) oneshot: bool,
    /// 加进来的目录（施工 5-10 上）：和工作区一样能读能写。
    #[serde(default)]
    pub(super) dirs: Vec<String>,
    /// 用哪个模型（施工 8-8）：模型或 `@池`；不写、写 `null` 的照这时的 `models.chat`。
    #[serde(default)]
    pub(super) model: Option<String>,
    /// 记忆的范围（施工 R-3 下）：`persona`、`session`、`off`；不写、写 `null` 的照人格的 `persona.toml`。
    #[serde(default)]
    pub(super) memory: Option<String>,
}

/// `session.list` 的参数（施工 3-9 下）。
#[derive(Debug, Deserialize)]
pub(super) struct ListParams {
    /// 只要一次性的。
    #[serde(default)]
    pub(super) oneshot: bool,
    /// 最多几个。
    #[serde(default)]
    pub(super) limit: Option<usize>,
}

/// `session.send` 的参数。
#[derive(Debug, Deserialize)]
pub(super) struct SendParams {
    pub(super) session: String,
    pub(super) text: String,
    #[serde(default)]
    pub(super) urgent: bool,
    /// 附件（施工 3-9 三补）：`blob.put` 的回应，照先后接在文字后面；不写、写 `null` 的是没有。
    #[serde(default)]
    pub(super) attachments: Option<Vec<Attachment>>,
    /// 别的 harness 报的名字（施工 7-10）：写了的，这一句是它说的；不写、写 `null` 的是本人。
    #[serde(default)]
    pub(super) from: Option<String>,
    /// 代表通讯平台上的人（施工 O-3，`venues.md`）：只给场所会话，场所会话也只收带它的。
    #[serde(default, rename = "as")]
    pub(super) as_external: Option<crate::venues::AsParams>,
}

/// `session.interrupt` 的参数。
#[derive(Debug, Deserialize)]
pub(super) struct InterruptParams {
    pub(super) session: String,
    pub(super) queued: QueuedParam,
}

/// `session.revert` 的参数（施工 4-7 上）：从哪一轮起撤，回合编号就是那一轮 `turn.started` 的序号；不写的撤
/// 最后一轮（施工 4-7 下）。
#[derive(Debug, Deserialize)]
pub(super) struct RevertParams {
    pub(super) session: String,
    #[serde(default)]
    pub(super) turn: Option<u64>,
}

/// `session.unrevert` 的参数（施工 4-7 上）。
#[derive(Debug, Deserialize)]
pub(super) struct UnrevertParams {
    pub(super) session: String,
}

/// `session.redo` 的参数（施工 4-7 再补）：开这一轮的那一句换成的话、换成的附件，写法照 `session.send`；不写、写 `null` 的
/// 照原来那一句的。
#[derive(Debug, Deserialize)]
pub(super) struct RedoParams {
    pub(super) session: String,
    #[serde(default)]
    pub(super) text: Option<String>,
    #[serde(default)]
    pub(super) attachments: Option<Vec<Attachment>>,
}

/// `session.clear` 的参数（施工 6-8 补）。
#[derive(Debug, Deserialize)]
pub(super) struct ClearParams {
    pub(super) session: String,
}

/// `session.recap` 的参数（施工 3-8 四补）。
#[derive(Debug, Deserialize)]
pub(super) struct RecapParams {
    pub(super) session: String,
}

/// `session.compact` 的参数（施工 6-8）：人附的要求可以不写，原样交给内核（只有空白的由内核当没写）。
#[derive(Debug, Deserialize)]
pub(super) struct CompactParams {
    pub(super) session: String,
    #[serde(default)]
    pub(super) instructions: Option<String>,
}

/// `session.set_permission_level` 的参数（施工 3-8 再补）：常用的那一级、只读开关，改哪样写哪样；两格都不写的是参数不对。
#[derive(Debug, Deserialize)]
pub(super) struct PermissionParams {
    pub(super) session: String,
    #[serde(default)]
    pub(super) level: Option<LevelParam>,
    #[serde(default)]
    pub(super) read_only: Option<bool>,
}

/// `session.answer` 的参数（施工 D-1，`protocol.md` 的 `session.answer`）：回答确认的写 `decision`，拒绝的可以带 `reason`；
/// 回答提问的写 `answers`，照题目的先后一道一条。
#[derive(Debug, Deserialize)]
pub(super) struct AnswerParams {
    pub(super) session: String,
    pub(super) call: String,
    #[serde(default)]
    decision: Option<DecisionParam>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    answers: Option<Vec<Response>>,
}

impl AnswerParams {
    /// 交给内核的回答。`decision`、`answers` 两样都写、都不写，回答提问却带了 `reason`：参数不对。
    pub(super) fn answer(self) -> Result<Answer, Refusal> {
        match (self.decision, self.answers) {
            (Some(decision), None) => Ok(Answer::Approval {
                decision: decision.into(),
                reason: self.reason,
            }),
            (None, Some(answers)) if self.reason.is_none() => Ok(Answer::Questions(answers)),
            _ => Err(Refusal::BAD_PARAMS),
        }
    }
}

/// 协议上回答确认能选的三项（施工 D-1）。「这个工作区以后都允许」2026-09-28 项目主人定以后再加，协议先不认，写了是参数
/// 不对：内核的 `unknown_decision` 因此从协议上碰不到。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DecisionParam {
    Once,
    Session,
    Deny,
}

impl From<DecisionParam> for Decision {
    fn from(decision: DecisionParam) -> Decision {
        match decision {
            DecisionParam::Once => Decision::Once,
            DecisionParam::Session => Decision::Session,
            DecisionParam::Deny => Decision::Deny,
        }
    }
}

/// `session.delete` 的参数（施工 3-8 三补）。
#[derive(Debug, Deserialize)]
pub(super) struct DeleteParams {
    pub(super) session: String,
}

/// 协议上能切到的常用的那一级。只认这两种，别的是参数不对：内核的 `unknown_level` 从协议上碰不到，和 `queued`、`stream`
/// 一样，值不在表里的算参数读不成（`protocol.md` 的 `session.set_permission_level`）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum LevelParam {
    Workspace,
    Full,
}

/// `job.stop` 的参数（施工 7-4）：哪个会话的哪个任务。
#[derive(Debug, Deserialize)]
pub(super) struct JobStopParams {
    pub(super) session: String,
    pub(super) job: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum QueuedParam {
    Send,
    Return,
    Keep,
}

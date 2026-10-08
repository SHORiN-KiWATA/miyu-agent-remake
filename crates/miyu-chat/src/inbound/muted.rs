//! 她被禁言了（`docs/blueprint/chat.md` 第二条「怎么走」第 3 条，`18-通讯平台.md` 第六节）：谁说的都只记下，她开不了口，
//! 放行也回不了话（施工时定的第 3 条）。禁言的状态由外面照平台的通知和复查交进来。

use super::{Clock, Ctx, Inbound, InboundRule, Outcome, Step, Why};

/// 禁言这条规则。
pub(super) struct Rule;

impl InboundRule for Rule {
    fn judge(&self, _msg: &Inbound, ctx: &Ctx, _clock: Clock) -> Step {
        match ctx.muted {
            true => Step::Stop(Outcome::RecordOnly(Why::Muted)),
            false => Step::Continue,
        }
    }
}

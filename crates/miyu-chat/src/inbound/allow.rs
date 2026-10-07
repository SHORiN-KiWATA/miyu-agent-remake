//! 谁能叫她（`docs/blueprint/chat.md` 第二条「怎么走」第 4 条，`18-通讯平台.md` 第三节、第六节）：场所规则的 `allow` 是
//! `false` 时，只放行主人和私聊里的自己人，别的只记下。没设 `allow` 当 `true`。

use super::{Clock, Ctx, Inbound, InboundRule, Outcome, Step, Why};

/// 谁能叫她这条规则。
pub(super) struct Rule;

impl InboundRule for Rule {
    fn judge(&self, msg: &Inbound, ctx: &Ctx, _clock: Clock) -> Step {
        match ctx.allow == Some(false) && !msg.excused() {
            true => Step::Stop(Outcome::RecordOnly(Why::NotAllowed)),
            false => Step::Continue,
        }
    }
}

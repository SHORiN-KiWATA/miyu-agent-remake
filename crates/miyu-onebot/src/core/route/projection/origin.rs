//! 叫她做的那条（施工 O-31，`onebot.md` 第一条「平台工具（一）」第 3 条）：投影交给平台工具的两样，照投影里记的人说的话、她发过的
//! 编号算，纯逻辑。从 `projection.rs` 分出来（那一份行数到了上限）。

use miyu_kernel::id::ExternalId;

use super::super::platform::{Origin, Quote};
use super::Projection;

impl Projection {
    /// 叫她做的那条（O-31，「平台工具（一）」第 3 条）：主线这一轮她回的那一条（同出站链的引用，和核心记 `tool.call` 的 `by`
    /// 一个取法），连同它的引用和 @。引用的是她发过的编号的算她的；别的照人说的话认发的人，认不出的不知道是谁。主线不在跑、
    /// 这一轮没有触发的是空的。
    pub(in crate::core::route) fn origin(&self) -> Option<Origin> {
        let aimed = self.said.get(&self.running.as_ref()?.aim?)?;
        let quote = aimed.reply_to.as_ref().map(|msg| Quote {
            msg: msg.clone(),
            mine: self.mine(msg),
            sender: self
                .said
                .values()
                .find(|speaker| speaker.msg.as_deref() == Some(msg))
                .map(|speaker| speaker.id.clone()),
        });
        Some(Origin {
            sender: aimed.id.clone(),
            quote,
            mentions: aimed.mentions.clone(),
        })
    }

    /// 平台上的 `who` 是不是终端管理员（O-31：禁言不动他）：他在这个群说过的话带 `account`（「施工时定的」第 183 条）。
    pub(in crate::core::route) fn is_admin(&self, who: &ExternalId) -> bool {
        self.said
            .values()
            .any(|speaker| speaker.admin && speaker.id == *who)
    }
}

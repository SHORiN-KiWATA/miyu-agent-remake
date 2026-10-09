//! 斜杠命令（施工 O-19，`onebot.md` 第一条「怎么走」第 7、8 条之间的「斜杠命令」，`venues.md`「斜杠命令」）：私聊、群（施工
//! O-22，「群消息」第 7 条）的文字去掉开头的空白以后以 `/` 开头的，先原样交核心的 `command.run`，和发消息同一个命令编号。
//! 核心认命令、判谁能用、执行、照连接的语言写好回执；桥只分三种回应：成了的回执发回去，核心认不出的（`unknown_command`）
//! 交回给 `Route::submit` 照普通的话发，别的被拒把核心拒绝时那一句（`error.message`，照连接的语言写好的，「施工时定的」
//! 第 31 条）发回去（群里的发回群里）、不交给她。发回去的交 `receipt`：入队（施工 O-25 中），群里的过几秒撤回（施工 O-25 上）。
//! 运行日志只记正名和原因码，不记原文。

use serde_json::Value;

use super::{Message, Route};
use crate::TARGET;
use crate::core::{Gone, reason};

impl Route {
    /// 把 `message` 当斜杠命令交给核心（「斜杠命令」第 1 到 5 条）。办完了（成了、被拒、不接、找不到会话）交回真；核心认不
    /// 出的交回假，由用的一方照普通的话发。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn command(&mut self, message: &Message) -> Result<bool, Gone> {
        let Some((session, reply)) = self.deliver(message, "command.run").await? else {
            return Ok(true);
        };
        let (venue, number) = (&message.place.peer.venue, message.number);
        let said = match reason(&reply) {
            None => {
                let command = reply["result"]["command"].as_str().unwrap_or_default();
                tracing::info!(target: TARGET, venue = %venue, message = number, command, "command ran");
                said(&reply["result"]["said"])
            }
            Some(reason @ "unknown_command") => {
                tracing::debug!(target: TARGET, venue = %venue, message = number, reason, "not a command");
                return Ok(false);
            }
            Some(reason) => {
                tracing::info!(target: TARGET, venue = %venue, message = number, reason, "command refused");
                said(&reply["error"]["message"])
            }
        };
        self.receipt(&session, said).await?;
        Ok(true)
    }
}

/// 像不像一条斜杠命令：去掉开头的空白（Unicode 的空白，和核心的认法一样）以后以 `/` 开头。是哪个命令由核心认（「施工时
/// 定的」第 32 条）。
pub(super) fn looks_like(text: &str) -> bool {
    text.trim_start().starts_with('/')
}

/// 回应里给人看的那一句；不是字的（照说不会）当空的，空的不发。
fn said(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

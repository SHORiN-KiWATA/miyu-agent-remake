//! 空闲的会话退下，把手这一头（施工 V-2 再补，`docs/designs/07-存储.md` 第七节「会话按需载入」第 3 条，
//! `docs/blueprint/session/actor.md`「退下」）：会话表拿着表的锁问，actor 照自己的账答（`actor/life.rs`）。

use std::time::Duration;

use super::{Handle, Message, Stopped};

/// 问「闲够了没有」的回答。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Retire {
    /// 退下了：actor 不再收信，会话表拿掉它，下次用到再载入。
    Retired,
    /// 别的都空着，只是闲得还不够久：还差这么久。
    Later(Duration),
    /// 还有事：回合、头看着、派出去还没回报的、记忆的闹钟、有会话等它空下来……过一阵再问。
    Kept,
}

/// 最多等 actor 答多久：它手里那一封办得慢的，这一次当它还有事，会话表不一直拿着锁等。
const ANSWER_WAIT: Duration = Duration::from_secs(1);

impl Handle {
    /// 除了这一个，没有别的把手（施工 V-2 再补）：正在办的请求、订阅着的头都拿着一个。会话表拿着表的锁看它：这一刻谁也
    /// 拿不到新的把手，问退下时没有第二个写者的可能。
    pub fn alone(&self) -> bool {
        self.inbox.strong_count() == 1
    }

    /// 闲了 `idle` 以上、什么事都没有的退下（施工 V-2 再补）。答得慢的叫它别答了，当还有事；那一刻已经答了的照答的算。
    /// 回不进来的 actor 不退：会话表还当它在跑。
    ///
    /// # Errors
    ///
    /// 会话已经停了。
    pub async fn retire(&self, idle: Duration) -> Result<Retire, Stopped> {
        let (reply, mut answer) = tokio::sync::oneshot::channel();
        self.send(Message::Retire { idle, reply })?;
        match tokio::time::timeout(ANSWER_WAIT, &mut answer).await {
            Ok(answered) => answered.map_err(|_| Stopped),
            Err(_) => {
                answer.close();
                Ok(answer.try_recv().unwrap_or(Retire::Kept))
            }
        }
    }
}

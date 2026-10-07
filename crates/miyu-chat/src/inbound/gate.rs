//! 回合闸（`docs/blueprint/chat.md` 第二条「怎么走」第 8 条，`18-通讯平台.md` 第六节、第十六节）：核心自己开的回合（子代理
//! 回报、后台命令结束、定时到了）开之前先问桥，桥照这里的回答。闸只能推迟，不能丢。
//!
//! 「先记着」不在这里：只有桥没连着时由核心自己定，群聊内核这时根本不在（施工时定的第 5 条）。

use super::{Clock, Ctx, rate, sleep};

/// 回合闸的回答。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// 现在开：醒着，额度也够。
    Now,
    /// 推迟到这个时刻（自 Unix 纪元起的毫秒）再问。
    Later(i64),
}

/// 回合闸的回答：睡着的推迟到醒来那一分钟的开头，额度满了的推迟到窗口里最早那个回合出窗口；两样都占取晚的，都不占现在开。
/// 核心开的回合没有发消息的人，睡眠不看人。
pub fn gate(ctx: &Ctx, clock: Clock) -> Gate {
    let woken = sleep::wakes(ctx, clock);
    let freed = rate::full(ctx, clock.now).map(|full| full.frees);
    // `None` 比哪个 `Some` 都小：取大的就是取晚的那一个。
    match woken.max(freed) {
        Some(at) => Gate::Later(at),
        None => Gate::Now,
    }
}

#[cfg(test)]
mod tests;

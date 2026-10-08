//! 顶替（`docs/blueprint/chat.md` 第四条「怎么走」第 1 到 4 条，`docs/designs/18-通讯平台.md` 第七节「顶替窗口」，
//! 施工 O-9）：主动回复判断的下半的前一半。同一个人在窗口里补发一条，前一条已经判过要回的，这一条接过去，不再判；前一条
//! 还在判的，取消它，几条一起重判（[`supersede`]）。
//!
//! 顶替不是一种触发条件，是合并条件的办法：合起来的集合照第三条取主触发、走路、算分，继承时原来的触发标签就留下了
//! （施工时定的第 1 条）。
//!
//! 纯逻辑：谁在判、谁判过要回（[`Pending`]），都由外面从场所会话的日志和桥的内存里投影出来交进来；取消判官请求随桥
//! （「怎么走」第 7 条）。分派在旁边的 `dispatch`，两样各一个文件（施工时定的第 9 条，O-12 下）。

use miyu_kernel::id::{ExternalId, Seq};
use miyu_kernel::time::Timestamp;

use crate::Clock;

use super::{Conditions, Facts, within};

/// 同一个场所里一条还没回完的消息：外面从场所会话的日志投影出来交进来。
#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    /// 这条消息的序号（场所主线会话日志里的，`chat.md` 第七条第 1 条）：判官请求挂在它上面，重判时取消的就是它
    /// （[`Supersede::Rejudge::cancel`]）。几条一起判的，是其中最后的一条。
    pub msg: Seq,
    /// 它早先接过的几条的序号，照先后，不含 [`Pending::msg`]：重判时连同它们一起带上（「怎么走」第 3 条）。没接过的
    /// 是空的。
    pub absorbed: Vec<Seq>,
    /// 发的人：平台上的人的编号，和这一条的 [`Said::sender`](crate::Said::sender) 比。
    pub sender: ExternalId,
    /// 它的时刻：[`Pending::msg`] 那一条进来的时刻，顶替窗口从它数。
    pub at: Timestamp,
    /// 判官还在判，还是判过要回、还没回完。
    pub status: Status,
    /// 它当时成立的条件；接过别的的，是合起来以后的。叫 `conditions` 不叫 `hits`：[`Conditions`] 里面那一格才是
    /// `hits`（施工时定的第 9 条，O-12 下）。
    pub conditions: Conditions,
}

/// 一条还没回完的消息走到哪一步了。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// 判官还在判：补发的一条取消它，几条一起重判。
    Judging,
    /// 判过要回（或者主人的 @ 直通），还没回完：补发的一条接过去，不再判。
    Committed,
}

/// 顶替的结论（「怎么走」第 1 到 4 条）。
#[derive(Debug, Clone, PartialEq)]
pub enum Supersede {
    /// 没有顶替：这一条照第三条自己走。
    None,
    /// 接过去，不再判：`msg` 是被接过的那一条的序号，`conditions` 是合起来的条件。
    Inherit {
        /// 被接过的那一条的序号（[`Pending::msg`]）。
        msg: Seq,
        /// 那一条的条件加上这一条自己成立的，同一种只留一个。
        conditions: Conditions,
    },
    /// 取消在判的那一条，几条一起重判。
    Rejudge {
        /// 要取消的判官请求挂在哪一条上（[`Pending::msg`]）。
        cancel: Seq,
        /// 一起重判的几条的序号，照先后：那一条接过的、那一条、这一条。
        msgs: Vec<Seq>,
        /// 那一条的条件加上这一条自己成立的，同一种只留一个。
        conditions: Conditions,
    },
}

/// 看这一条顶替了同一个人前面的哪一条（「怎么走」第 1 到 4 条）。`conditions` 是这一条自己成立的条件，`pendings` 是同一个场所
/// 里还没回完的，先后不要紧；`window` 是顶替窗口的毫秒数，出厂 7 秒。
///
/// - 找谁：`pendings` 里发的人和这一条一样、`0 ≤ now − at < window` 的，取最晚的一条；同一毫秒的取交进来靠后的（同
///   第三条施工时定的第 6 条）。没有就是 [`Supersede::None`]。
/// - 那一条判过要回：[`Supersede::Inherit`]；还在判：[`Supersede::Rejudge`]。
/// - 条件：那一条的在前，这一条新添的种类跟在后面，同一种只留那一条的（施工时定的第 2 条）。
///
/// 这一条自己一个条件都没有也照样顶替：设计里顶替本是一个加值项，同一个人在窗口里补发就成立（18 第七节那张表），所以
/// 那张流程图「集合是空的？」问的集合里已经算上了它；发错了马上改的那一条常常没 @ 她（施工时定的第 4 条）。
pub fn supersede(
    facts: &Facts,
    conditions: &Conditions,
    pendings: &[Pending],
    clock: Clock,
    window: i64,
) -> Supersede {
    let before = pendings
        .iter()
        .filter(|pending| {
            pending.sender == facts.said.sender && within(pending.at, clock.now, window)
        })
        .max_by_key(|pending| pending.at);
    let Some(before) = before else {
        return Supersede::None;
    };
    let conditions = merge(&before.conditions, conditions);
    match before.status {
        Status::Committed => Supersede::Inherit {
            msg: before.msg,
            conditions,
        },
        Status::Judging => {
            let mut msgs = before.absorbed.clone();
            msgs.push(before.msg);
            msgs.push(facts.msg);
            Supersede::Rejudge {
                cancel: before.msg,
                msgs,
                conditions,
            }
        }
    }
}

/// 前一条的条件加上这一条的，同一种只留前一条的那一笔：几条补发不该把同一笔加分加好几次。
fn merge(before: &Conditions, own: &Conditions) -> Conditions {
    let mut hits = before.hits.clone();
    for hit in &own.hits {
        if !hits.iter().any(|kept| kept.kind == hit.kind) {
            hits.push(*hit);
        }
    }
    Conditions { hits }
}

#[cfg(test)]
mod tests;

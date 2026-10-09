//! 判官回来了（施工 O-23 下，`onebot.md` 第一条「群里怎么叫她」第 7、11、13 条）：问判官的任务交回号和回答，照号拿回在判的那
//! 一条；被顶替放下的拿不到，回答丢掉（施工单「要定的」第 1 条）。没被放下的先把这个会话留着的推送收进投影，读出来了的算分
//! （`score`：合起来的条件、投影里她的回复、判官回来的这一刻、交出去时套的那一份参数，「施工时定的」第 100 条），记一笔判断，
//! 回的开一轮；判不了的照不回算，只记下，运行日志另记一行为什么。

use miyu_chat::score;

use super::Route;
use super::applied;
use super::body::{Judged, unjudged_name};
use super::judges::Asked;
use crate::TARGET;
use crate::core::Gone;

impl Route {
    /// 号是 `ticket` 的那一次问完了（`answer`；核心断开了的是空的，什么都不做）。
    ///
    /// # Errors
    ///
    /// 写不出去、等的时候核心断开。
    pub(super) async fn judged(&mut self, (ticket, answer): Asked) -> Result<(), Gone> {
        let Some(answer) = answer else {
            return Ok(());
        };
        let Some(judging) = self.judges.take(ticket) else {
            tracing::debug!(target: TARGET, ticket, "judge answer dropped");
            return Ok(());
        };
        let session = judging.session.clone();
        if !self.groups.contains_key(&session) {
            // 这个群的会话不在了（再找的时候忘掉了投影）：判过的这一条已经不在跟着的日志里，不记。
            tracing::debug!(target: TARGET, venue = %judging.tag.venue, message = judging.tag.number, "judge answer dropped");
            return Ok(());
        }
        self.catch_up(&session).await;
        let scored = match (&answer.result, applied::clock(), self.groups.get(&session)) {
            (Ok(judgement), Some(clock), Some(group)) => Some(score(
                judgement,
                &judging.pending.conditions,
                group.replies(),
                clock,
                &judging.chatty,
            )),
            (Ok(_), None, _) => {
                tracing::warn!(target: TARGET, venue = %judging.tag.venue, message = judging.tag.number, "clock not readable, not scored");
                None
            }
            _ => None,
        };
        if let Err(unjudged) = &answer.result {
            let (why, _) = unjudged_name(unjudged);
            tracing::info!(target: TARGET, venue = %judging.tag.venue, message = judging.tag.number, why, tries = answer.tries, "not judged");
        }
        let judged = Judged {
            answer: &answer,
            score: scored.as_ref(),
        };
        self.settle(
            &session,
            &judging.decision,
            judging.standing,
            Some(judged),
            &judging.tag,
        )
        .await
    }
}

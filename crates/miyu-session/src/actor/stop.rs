//! 有计划地停下（`docs/blueprint/session/actor.md` 第 9 条）：送进「要重启了」；这个会话在跑的后台命令各记一条
//! `restarted`，落了盘再整组杀掉（施工 7-3，`agents.md` 第八条第 2 条）；回一声。

use std::collections::VecDeque;

use tokio::sync::oneshot;

use miyu_kernel::session::Input;

use super::{Actor, answer};
use crate::TARGET;
use crate::port::Back;

impl Actor {
    /// 有计划地停下：先把在跑的后台命令记成报了（之后它们自己退出了也不再报），收件箱里已经到了的结束一起交进去，排在
    /// 「要重启了」后面：内核这时只记下、不开轮。都落了盘，整组杀掉，回一声。写不进去的，actor 照常停下，丢掉任务表的那
    /// 一份时整组杀掉。
    pub(super) async fn stop(&mut self, reply: oneshot::Sender<()>) {
        let at = self.clock.now();
        let restarted = self.jobs.restarted(at).await;
        let mut inputs = VecDeque::from([Input::Restarting { at }]);
        // 别的回报不要了：要重启了，内核也不再理在路上的请求、工具。
        while let Ok(back) = self.back.try_recv() {
            if let Back::Job(ended) = back {
                inputs.push_back(self.jobs.arrived(at, ended));
            }
        }
        inputs.extend(restarted);
        if self.drain(inputs).await.is_ok() {
            self.jobs.close().await;
            tracing::info!(target: TARGET, "stopped");
            answer(reply, ());
        }
    }
}

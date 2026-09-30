//! 有计划地停下（`docs/blueprint/session/actor.md` 第 9 条）：送进「要重启了」；这个会话在跑的后台命令各记一条
//! `restarted`，落了盘再整组杀掉（施工 7-3，`agents.md` 第八条第 2 条）；回一声。
//!
//! 删会话之前停下（施工 3-8 三补）：问内核删不删得了，删得了的后台命令整组杀掉、不记，放开日志，回一声。

use std::collections::VecDeque;

use tokio::sync::oneshot;

use miyu_kernel::session::{Input, Reason};

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

    /// 删会话之前停下：`force` 是假的，先问内核删不删得了（`Session::deletable`），删不了的交回原因、照常跑，交回假。删得
    /// 了、或者 `force`（父会话被删，子会话一起停）：不再收新的后台命令，在跑的整组杀掉、不记回报；放开日志的文件，才回一声，
    /// 交回真，actor 退出。别的都随 actor 退出放下：路上的请求叫停，在跑的工具掐掉，收件箱里没办的回「会话停了」。
    pub(super) async fn delete(
        &mut self,
        force: bool,
        reply: oneshot::Sender<Result<(), Reason>>,
    ) -> bool {
        if !force && let Err(reason) = self.session.deletable() {
            answer(reply, Err(reason));
            return false;
        }
        self.jobs.close().await;
        // 先关日志再回：会话表一收到就挪会话目录，Windows 上开着的文件挪不走。
        self.store = None;
        tracing::info!(target: TARGET, "stopped for deletion");
        answer(reply, Ok(()));
        true
    }
}

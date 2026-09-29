//! 看守查推给头的瞬时事件（施工 7-2 从 `watch.rs` 挪出来，那边放不下了）：增量是在路上的那次请求的；工具的输出是在
//! 跑的调用的；重试的状态、压缩的进度照各自的规矩查。

use super::*;

impl Watch {
    /// 推给头的：增量是在路上的那次请求的；工具的输出是在跑的调用的。
    pub(super) fn transient(&mut self, transient: &Transient) {
        let seed = self.seed;
        self.transient_turn(transient);
        match &transient.body {
            TransientBody::ModelDelta(delta) => {
                self.seen_paths.insert("推了增量");
                self.not_summarizing(delta.seen);
                assert_eq!(
                    Some(delta.seen),
                    self.asking,
                    "种子 {seed}：推的增量不是在路上的那次请求的"
                );
                assert!(
                    self.sent.contains(&delta.seen),
                    "种子 {seed}：请求还没发出去就推了增量"
                );
                assert!(matches!(transient.by, By::Model(_)));
            }
            TransientBody::ToolProgress(progress) => {
                self.seen_paths.insert("推了工具的输出");
                assert!(
                    self.running.contains(&progress.call_id),
                    "种子 {seed}：{} 不在跑，却推了它的输出",
                    progress.call_id
                );
            }
            TransientBody::Status(status) => self.retry_status(status),
            TransientBody::CompactionProgress(progress) => self.compaction_progress(progress),
            TransientBody::CompactionDone(done) => self.compaction_done(transient.turn, done),
        }
    }
}

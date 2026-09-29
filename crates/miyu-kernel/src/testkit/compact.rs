//! 替身的压缩（施工 6-9 从 `stage.rs` 挪出来，那边放不下了）：手动压缩（6-8）做好以前，替身单开一轮顶着。

use super::Stage;
use crate::event::{Body, ContextCompacted, EndReason, Event, TurnEnded, TurnStarted};
use crate::id::{Seq, TurnId};
use crate::origin::By;

impl Stage {
    /// 压缩：替代到「磁盘」上原来的最后一条，检查点写 `summary`。手动压缩（6-8）做好以前这里先顶着：往「磁盘」追加
    /// 单开的一轮（`turn.started` 由原来的最后一条触发、`context.compacted`、`turn.ended`），`by` 都是内核，再载入会话
    /// （`08-上下文投影.md` 第七节「测试门禁」）。压缩一定带着它所在的回合：撤掉这一轮，压缩跟着撤掉（施工 6-9）。
    ///
    /// # Panics
    ///
    /// 有回合在进行：单开的一轮开不了。
    pub fn compact(&mut self, summary: &str) {
        let started = self
            .log
            .iter()
            .rposition(|event| matches!(event.body, Body::TurnStarted(_)));
        let ended = self
            .log
            .iter()
            .rposition(|event| matches!(event.body, Body::TurnEnded(_)));
        assert!(started <= ended, "有回合在进行，替身的压缩单开不了一轮");
        let upto = self
            .log
            .last()
            .map(|event| event.seq)
            .unwrap_or_else(|| panic!("日志是空的"));
        let at = self.tick();
        let turn = TurnId::new(upto.next());
        let event = |seq: Seq, body: Body| Event {
            seq,
            at,
            turn: Some(turn),
            by: By::Kernel,
            cause: None,
            body,
        };
        let opened = event(
            turn.started(),
            Body::TurnStarted(TurnStarted {
                trigger: upto,
                cwd: Some(self.environment.cwd.clone()),
                dirs: Vec::new(),
            }),
        );
        let compacted = event(
            turn.started().next(),
            Body::ContextCompacted(ContextCompacted {
                upto,
                summary: summary.to_string(),
                trigger: None,
                notes: String::new(),
                restored: Vec::new(),
                refills: None,
            }),
        );
        let closed = event(
            compacted.seq.next(),
            Body::TurnEnded(TurnEnded {
                reason: EndReason::Completed,
            }),
        );
        self.log.extend([opened, compacted, closed]);
        self.reload();
    }
}

//! 看守查排队的消息（`docs/designs/02-内核.md` 第六节「排队的消息」）：回合结束时还有排着队的，
//! 下一条就是由最后那条触发的新回合，没有的就不接着开；退回时撤回的，正好是排着队的那几条。

use super::*;

impl Watch {
    /// 这一批里的第 `k` 条，照排队的规矩查。
    pub(super) fn queue_check(&mut self, events: &[Event], k: usize) {
        let seed = self.seed;
        let event = &events[k];
        match &event.body {
            Body::MessageUser(_) if event.turn.is_some() => self.queued.push(event.seq),
            Body::ModelCalled(called) => self.queued.retain(|queued| *queued > called.seen),
            Body::MessageWithdrawn(withdrawn) => {
                self.seen_paths.insert("打断后排队的退回");
                assert_eq!(
                    self.interrupting,
                    Some(Queued::Return),
                    "种子 {seed}：只有退回的打断才撤回"
                );
                assert_eq!(
                    withdrawn.messages, self.queued,
                    "种子 {seed}：撤回的应该正好是排着队的那几条"
                );
                self.queued.clear();
            }
            Body::TurnEnded(ended) => {
                let next = events.get(k + 1).map(|event| &event.body);
                match self.queued.last() {
                    Some(&last) => {
                        assert!(
                            matches!(next, Some(Body::TurnStarted(started)) if started.trigger == last),
                            "种子 {seed}：还有排着队的 {last}，回合结束后应该由它接着开一轮"
                        );
                        self.seen_paths
                            .insert(if ended.reason == EndReason::Interrupted {
                                "打断后排队的接着发"
                            } else {
                                "排队的接着开了一轮"
                            });
                    }
                    None => assert!(
                        !matches!(next, Some(Body::TurnStarted(_))),
                        "种子 {seed}：没有排着队的，不该接着开"
                    ),
                }
                self.queued.clear();
            }
            _ => {}
        }
    }
}

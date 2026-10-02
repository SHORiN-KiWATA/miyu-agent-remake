//! 主循环下一次要自己醒来的时刻（`main.rs` 照它等）。

use std::time::{Duration, Instant};

use super::App;

impl App {
    /// 下一次要自己醒来的时刻：提示到点消失；在跑时每秒走一下用时。没有就是 `None`，一直等事件。
    pub fn deadline(&self) -> Option<Instant> {
        let notice = self.notice.as_ref().map(|n| n.until);
        let clock = self.transcript.running.map(|start| {
            let next = start.elapsed().as_secs() + 1;
            start + Duration::from_secs(next)
        });
        // 有步骤在转圈、运行状态行的流光在走，照转圈的节拍重画。
        // 会话列表开着、里面有在跑的：它标题前面的转圈也要转（`ui/session_list.rs`）。
        let listing_busy = self.panel == Some(super::Panel::Sessions)
            && self
                .session_list
                .as_ref()
                .is_some_and(|l| l.all.iter().any(|s| s.busy));
        let moving = self.transcript.busy() || self.transcript.running.is_some() || listing_busy;
        let spin =
            moving.then(|| Instant::now() + Duration::from_millis(self.config.timeline.spinner_ms));
        notice
            .into_iter()
            .chain(clock)
            .chain(spin)
            .chain(self.mascot_deadline(Instant::now()))
            .chain(self.jobs_deadline())
            .chain(self.drawer_deadline())
            // 核心的清单在建：到点再问（「`@` 文件列表」第 2 条）。
            .chain(self.mention.deadline())
            // 链接卡片、mermaid 图的单子上有没发的：马上醒来发（`cards.rs`、`diagrams.rs`）。
            .chain((self.card_asks_pending() || self.diagram_asks_pending()).then(Instant::now))
            // 整份重排没排完的：下一帧接着排（蓝图「正文」第 8 条）。
            .chain((self.row_cache.borrow().stale > 0).then(Instant::now))
            .min()
    }
}

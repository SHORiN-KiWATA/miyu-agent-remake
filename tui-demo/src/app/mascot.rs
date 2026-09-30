//! 吉祥物要界面这一头办的（蓝图 `tui.md`「空会话的首页」第 6–9 条）：什么时候醒来画下一帧。

use std::time::{Duration, Instant};

use super::App;

impl App {
    /// 吉祥物画着的时候，下一次该画的时刻：转头、待机小动作、鼠标停够了转回来、走下来、嘴在动。
    pub(super) fn mascot_deadline(&self, now: Instant) -> Vec<Instant> {
        if !self.mascot_shown() {
            return Vec::new();
        }
        let look = &self.config.mascot;
        // 转头：照它的节拍画，转到了就停（第 7 条）。
        let turning = self
            .gaze
            .moving()
            .then(|| now + Duration::from_millis(look.gaze.frame_ms));
        // 框里有字、鼠标停够了：到点画一帧，转回来看输入光标（第 6 条）。
        let settle = Duration::from_millis(look.gaze.pointer_settle_ms);
        let settling = (!self.input.editor.is_empty())
            .then(|| self.attention.settles_at(settle))
            .flatten()
            .filter(|at| *at > now);
        let walking = self
            .home()
            .then(|| self.perch.wake(now, &look.perch))
            .flatten();
        [
            turning,
            self.idle.wake(now, &look.idle),
            settling,
            walking,
            self.mouth.wake(now, &look.mouth),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

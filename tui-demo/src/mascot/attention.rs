//! 吉祥物看鼠标还是看输入光标（蓝图 `tui.md`「空会话的首页」第 6 条）：鼠标最近动过比框里有字优先。框里有字时一动
//! 鼠标就看鼠标，鼠标停下以后再按一下键、或者停够一会儿，回来看输入光标。

use std::time::{Duration, Instant};

/// 最近一次动鼠标、按键的时刻。
#[derive(Debug, Default, Clone, Copy)]
pub struct Attention {
    pointed: Option<Instant>,
    typed: Option<Instant>,
}

impl Attention {
    /// 鼠标动了。
    pub fn pointed(&mut self, now: Instant) {
        self.pointed = Some(now);
    }

    /// 按了键、粘了东西。
    pub fn typed(&mut self, now: Instant) {
        self.typed = Some(now);
    }

    /// 该看鼠标：鼠标比按键动得晚，而且还没停够 `settle`。
    pub fn on_pointer(&self, now: Instant, settle: Duration) -> bool {
        self.settles_at(settle).is_some_and(|at| now < at)
    }

    /// 鼠标停够、该回来看光标的那一刻：鼠标比按键动得晚才有（主循环到点醒来画一帧）。
    pub fn settles_at(&self, settle: Duration) -> Option<Instant> {
        let pointed = self.pointed?;
        let newer = self.typed.is_none_or(|typed| pointed > typed);
        newer.then_some(pointed + settle)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::Attention;

    #[test]
    fn a_moving_pointer_wins_until_a_key_or_a_pause() {
        // 2026-09-30 项目主人：框里有字时动鼠标也要跟着鼠标看，停下以后再回来看光标。
        let settle = Duration::from_millis(1500);
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut a = Attention::default();
        assert!(!a.on_pointer(t0, settle), "没动过鼠标");
        a.typed(at(0));
        a.pointed(at(100));
        assert!(a.on_pointer(at(200), settle), "鼠标比按键晚：看鼠标");
        assert!(!a.on_pointer(at(1700), settle), "停够了：回来看光标");
        assert_eq!(a.settles_at(settle), Some(at(1600)), "到点醒来画一帧");
        a.typed(at(300));
        assert!(!a.on_pointer(at(400), settle), "又按了键：看光标");
        assert_eq!(a.settles_at(settle), None);
    }
}

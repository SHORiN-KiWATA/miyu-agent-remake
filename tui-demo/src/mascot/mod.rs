//! 首页的吉祥物（蓝图 `tui.md`「空会话的首页」第 5–7 条）：模型的数值在 `model.rs`，逐格打光线在 `render.rs`，
//! 转头在这里：照目标定角度，缓动过去。角度都是度，左右 `yaw` 往右为正，上下 `pitch` 往下为正。

mod idle;
mod model;
mod perch;
mod render;

use std::time::{Duration, Instant};

pub use idle::Idle;
pub use model::{Gaze as GazeLook, Look, Part};
pub use perch::Perch;
pub use render::render;

/// 这一帧的样子：朝哪、闭没闭眼、耳朵往外多歪多少（弧度）。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pose {
    /// 左右，度，往右为正。
    pub yaw: f64,
    /// 上下，度，往下为正。
    pub pitch: f64,
    /// 闭着眼。
    pub blink: bool,
    /// 耳朵往外多歪多少。
    pub ear: f64,
}

impl Pose {
    /// 睁着眼、耳朵不动，朝 `yaw`、`pitch`。
    pub fn facing(yaw: f64, pitch: f64) -> Self {
        Self {
            yaw,
            pitch,
            ..Self::default()
        }
    }
}

/// 转头：现在朝哪、要转到哪。
#[derive(Debug, Clone, Default)]
pub struct Gaze {
    now: (f64, f64),
    target: (f64, f64),
    at: Option<Instant>,
}

impl Gaze {
    /// 定下要转到哪。
    pub fn aim(&mut self, target: (f64, f64)) {
        self.target = target;
    }

    /// 走到 `now` 这一刻：每过一个半衰期走剩下的一半，一帧最多走一个半衰期；差不到 0.2° 就停在目标上。
    pub fn step(&mut self, now: Instant, half_life: Duration) {
        let dt = self
            .at
            .map_or(Duration::ZERO, |at| now.saturating_duration_since(at))
            .min(half_life);
        self.at = Some(now);
        let keep = if half_life.is_zero() {
            0.0
        } else {
            0.5_f64.powf(dt.as_secs_f64() / half_life.as_secs_f64())
        };
        let go = |from: f64, to: f64| {
            let next = to + (from - to) * keep;
            if (next - to).abs() < 0.2 { to } else { next }
        };
        self.now = (go(self.now.0, self.target.0), go(self.now.1, self.target.1));
    }

    /// 现在的角度：`(yaw, pitch)`。
    pub fn angles(&self) -> (f64, f64) {
        self.now
    }

    /// 还在转：没到目标。
    pub fn moving(&self) -> bool {
        self.now != self.target
    }
}

/// 从脸的中心 `from`（列、行，带小数）看 `to` 那一格，要转多少。竖着一格按 `aspect` 列算。
pub fn toward(from: (f64, f64), to: (f64, f64), aspect: f64, gaze: &GazeLook) -> (f64, f64) {
    let across = to.0 - from.0;
    let down = (to.1 - from.1) * aspect;
    let yaw = across.atan2(gaze.distance).to_degrees();
    let pitch = down.atan2(gaze.distance).to_degrees();
    (
        yaw.clamp(-gaze.max_yaw, gaze.max_yaw),
        pitch.clamp(-gaze.max_pitch, gaze.max_pitch),
    )
}

#[cfg(test)]
mod tests;
